using System.Windows;
using System.Windows.Threading;
using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Reflection;
using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Control;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Core.AppLifecycle;
using SwitchifyPc.Core.Diagnostics;
using SwitchifyPc.Core.Startup;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;
using SwitchifyPc.App.Themes;
using SwitchifyPc.Windows.AppLifecycle;
using SwitchifyPc.Windows.Bluetooth;
using SwitchifyPc.Windows.CursorOverlay;
using SwitchifyPc.Windows.Input;
using SwitchifyPc.Windows.ModifierOverlay;
using SwitchifyPc.Windows.Startup;
using SwitchifyPc.Windows.Updates;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.App;

public partial class App : System.Windows.Application
{
    private static readonly string CurrentVersion = typeof(App).Assembly
        .GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion
            .Split('+', 2, StringSplitOptions.RemoveEmptyEntries)[0] ?? "0.2.0";

    private SingleInstanceService? singleInstance;
    private WindowsExistingInstanceSignal? existingInstanceSignal;
    private NativeTrayIcon? trayIcon;
    private SettingsWindow? settingsWindow;
    private SetupGuideWindow? setupGuideWindow;
    private MainWindowViewModel mainWindowViewModel = new();
    private SetupGuideViewModel setupGuideViewModel = new();
    private UpdateService? updateService;
    private PairingApprovalManager? pairingApprovalManager;
    private BluetoothStatusTracker? bluetoothStatusTracker;
    private WindowsBluetoothGattServer? bluetoothServer;
    private BluetoothRemoteFrameProcessor? bluetoothFrameProcessor;
    private DesktopCommandExecutor? commandExecutor;
    private MouseRepeatController? mouseRepeatController;
    private WindowsCursorOverlayNotifier? cursorOverlay;
    private WindowsModifierKeyOverlayNotifier? modifierOverlay;
    private readonly SemaphoreSlim bluetoothMessageProcessing = new(1, 1);
    private readonly object bluetoothConnectionSync = new();
    private readonly HashSet<string> authenticatedBluetoothConnections = new(StringComparer.Ordinal);
    private DispatcherTimer? pairingExpiryTimer;
    private DispatcherTimer? telemetryFlushTimer;
    private AppThemeManager? themeManager;
    private JsonTelemetrySettingsStore? telemetrySettingsStore;
    private ITelemetryReporter? telemetryReporter;
    private readonly HashSet<Exception> reportedExceptions = new(ReferenceEqualityComparer.Instance);
    private readonly object reportedExceptionsSync = new();
    private bool diagnosticsConsentShown;
    private string? lastLoggedUpdateCheckStatus;
    private string? lastLoggedUpdateDownloadStatus;
    private bool isQuitting;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        ShutdownMode = ShutdownMode.OnExplicitShutdown;
        singleInstance = new SingleInstanceService(new WindowsSingleInstanceLockFactory());
        AppLaunchOptions launchOptions = AppLaunchOptions.Parse(e.Args);
        SingleInstanceDecision decision = singleInstance.Start(launchOptions);

        if (!decision.IsPrimaryInstance)
        {
            using WindowsExistingInstanceSignal signal = new();
            if (decision.ExistingInstanceAction == ExistingInstanceAction.ShowMainWindow)
            {
                signal.SignalShowMainWindow();
            }
            else if (decision.ExistingInstanceAction == ExistingInstanceAction.QuitForInstall)
            {
                signal.SignalQuitForInstall();
            }

            Shutdown();
            return;
        }

        InitializeTelemetry();

        existingInstanceSignal = new WindowsExistingInstanceSignal();
        existingInstanceSignal.Start(
            () => Dispatcher.BeginInvoke(ShowMainWindow),
            () => Dispatcher.BeginInvoke(QuitApplication));
        themeManager = AppThemeManager.ForCurrentApplication(new WindowsAppThemeProvider());
        themeManager.Start();
        updateService = CreateUpdateService();
        pairingApprovalManager = CreatePairingApprovalManager();
        bluetoothStatusTracker = new BluetoothStatusTracker(onStatusChanged: UpdateBluetoothState);
        RefreshPairingApprovals();
        updateService.StartAutomaticUpdateChecks();
        StartPairingExpiryTimer();
        StartTelemetryFlushTimer();
        _ = telemetryReporter?.FlushAsync();
        _ = StartBluetoothAsync();
        _ = InitializeStartupRegistrationAsync(e.Args, launchOptions.StartHidden);
        trayIcon = new NativeTrayIcon(
            ShowMainWindow,
            ShowSettingsWindow,
            TrayStatusText,
            CanDisconnectBluetoothDevices,
            DisconnectBluetoothDevices,
            QuitApplication);

        if (decision.ShowMainWindow)
        {
            ShowMainWindow();
        }

        RecordRuntimeDiagnostic("app.startup.completed", status: decision.ShowMainWindow ? "window" : "tray");
    }

    protected override void OnExit(ExitEventArgs e)
    {
        RecordRuntimeDiagnostic("app.exit");
        singleInstance?.Stop();
        singleInstance = null;
        existingInstanceSignal?.Dispose();
        existingInstanceSignal = null;
        updateService?.StopAutomaticUpdateChecks();
        updateService = null;
        pairingExpiryTimer?.Stop();
        pairingExpiryTimer = null;
        telemetryFlushTimer?.Stop();
        telemetryFlushTimer = null;
        mouseRepeatController?.StopAllAsync().GetAwaiter().GetResult();
        mouseRepeatController = null;
        bluetoothServer?.Dispose();
        bluetoothServer = null;
        cursorOverlay?.Dispose();
        cursorOverlay = null;
        modifierOverlay?.Dispose();
        modifierOverlay = null;
        themeManager?.Dispose();
        themeManager = null;
        commandExecutor = null;
        bluetoothFrameProcessor = null;
        bluetoothStatusTracker = null;
        pairingApprovalManager = null;
        lock (bluetoothConnectionSync)
        {
            authenticatedBluetoothConnections.Clear();
        }
        trayIcon?.Dispose();
        trayIcon = null;
        settingsWindow = null;
        setupGuideWindow = null;
        telemetryReporter?.Dispose();
        telemetryReporter = null;
        telemetrySettingsStore = null;
        base.OnExit(e);
    }

    private void ShowMainWindow()
    {
        Window window = MainWindow ?? CreateMainWindow();
        MainWindow = window;

        window.Show();
        window.WindowState = WindowState.Normal;
        window.Activate();
        _ = RefreshSetupGuideStateAsync(allowAutoOpen: true);
    }

    private Window CreateMainWindow()
    {
        mainWindowViewModel.SetUpdateState(updateService?.GetState() ?? UpdateState.CreateInitial(CurrentVersion));
        RefreshPairingApprovals();
        SwitchifyPc.App.MainWindow window = new(
            mainWindowViewModel,
            ShowSettingsWindow,
            ShowSetupGuideWindow,
            ShowSettingsWindow,
            DisconnectBluetoothDevices,
            AcceptPairingApprovalAsync,
            RejectPairingApproval);
        window.Closing += (_, eventArgs) =>
        {
            if (isQuitting) return;
            eventArgs.Cancel = true;
            window.Hide();
        };

        return window;
    }

    private void QuitApplication()
    {
        isQuitting = true;
        Shutdown();
    }

    private string TrayStatusText()
    {
        BluetoothStatus status = bluetoothStatusTracker?.Status ?? BluetoothStatusModel.DefaultStatus;
        return $"Status: {MainWindowCopy.BluetoothStatusLabel(status)}";
    }

    private bool CanDisconnectBluetoothDevices()
    {
        return bluetoothStatusTracker?.Status.ConnectedClientCount > 0;
    }

    private void DisconnectBluetoothDevices()
    {
        bluetoothServer?.DisconnectAll();
    }

    private void ShowSettingsWindow()
    {
        settingsWindow ??= CreateSettingsWindow();
        CenterWindow(settingsWindow);
        settingsWindow.Show();
        settingsWindow.WindowState = WindowState.Normal;
        settingsWindow.Activate();
    }

    private void ShowSetupGuideWindow()
    {
        MarkSetupGuideShown();
        ShowSetupGuideWindowCore();
        _ = RefreshSetupGuideStateAsync(allowAutoOpen: false);
    }

    private void ShowSetupGuideWindowCore()
    {
        setupGuideWindow ??= CreateSetupGuideWindow();
        setupGuideWindow.Owner = MainWindow;
        setupGuideWindow.Show();
        setupGuideWindow.WindowState = WindowState.Normal;
        setupGuideWindow.Activate();
    }

    private SetupGuideWindow CreateSetupGuideWindow()
    {
        SetupGuideWindow window = new(
            setupGuideViewModel,
            OpenAndroidDownloadUrl,
            AcceptPairingApprovalAsync,
            RejectPairingApproval,
            SetSetupStartWithSystemAsync,
            CompleteSetupGuide,
            () => Dispatcher.BeginInvoke(ShowDiagnosticsConsentIfNeeded),
            setShareDiagnosticData: SetShareDiagnosticData);
        window.Closing += (_, eventArgs) =>
        {
            if (isQuitting) return;
            eventArgs.Cancel = true;
            window.Hide();
        };
        return window;
    }

    private SettingsWindow CreateSettingsWindow()
    {
        SettingsWindow window = new(CreateSettingsController());
        window.Closing += (_, eventArgs) =>
        {
            if (isQuitting) return;
            eventArgs.Cancel = true;
            window.Hide();
        };

        return window;
    }

    private static void CenterWindow(Window window)
    {
        window.Left = SystemParameters.WorkArea.Left + (SystemParameters.WorkArea.Width - window.Width) / 2;
        window.Top = SystemParameters.WorkArea.Top + (SystemParameters.WorkArea.Height - window.Height) / 2;
    }

    private SettingsController CreateSettingsController()
    {
        SettingsViewModel viewModel = new();
        string userDataDirectory = UserDataDirectory();
        return new SettingsController(
            viewModel,
            CreateStartupService(),
            new JsonPointerMovementSettingsStore(Path.Combine(userDataDirectory, "pointer-movement-settings.json")),
            new JsonMouseRepeatSettingsStore(Path.Combine(userDataDirectory, "mouse-repeat-settings.json")),
            new JsonCursorOverlaySettingsStore(Path.Combine(userDataDirectory, "cursor-overlay-settings.json")),
            updateService ?? CreateUpdateService(),
            new JsonPairingStore(Path.Combine(userDataDirectory, "pairing-state.json")),
            telemetrySettingsStore ?? CreateTelemetrySettingsStore(),
            telemetryReporter ?? CreateDisabledTelemetryReporter());
    }

    private SystemStartupService CreateStartupService()
    {
        string mainExecutablePath = Environment.ProcessPath ?? string.Empty;
        string installDirectory = Path.GetDirectoryName(mainExecutablePath) ?? string.Empty;
        return new SystemStartupService(
            platform: "win32",
            isPackaged: IsInstalledApp(),
            mainExecutablePath: mainExecutablePath,
            startupLauncherPath: Path.Combine(installDirectory, SystemStartupService.StartupLauncherFileName),
            startupTask: new WindowsStartupTask(),
            startupRegistry: new WindowsStartupRegistry(),
            startupCommandFor: WindowsStartupRegistry.StartupCommandFor);
    }

    private UpdateService CreateUpdateService()
    {
        string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        string cacheDirectory = Path.Combine(localAppData, "switchify-pc-updater");
        return new UpdateService(new UpdateServiceOptions(
            CurrentVersion: CurrentVersion,
            IsPackaged: IsInstalledApp(),
            Platform: "win32",
            Backend: new GitHubReleaseUpdateBackend(new HttpClient(), "switchifyapp", "switchify-pc", cacheDirectory),
            InstallerLauncher: new WindowsUpdateInstallerLauncher(),
            OnStateChanged: UpdateMainWindowState));
    }

    private PairingApprovalManager CreatePairingApprovalManager()
    {
        return new PairingApprovalManager(new JsonPairingStore(Path.Combine(UserDataDirectory(), "pairing-state.json")));
    }

    private JsonMainWindowPromptSettingsStore CreateMainWindowPromptSettingsStore()
    {
        return new JsonMainWindowPromptSettingsStore(Path.Combine(UserDataDirectory(), "main-window-prompt-settings.json"));
    }

    private async Task StartBluetoothAsync()
    {
        try
        {
            bluetoothStatusTracker?.SetStarting();
            RecordRuntimeDiagnostic("bluetooth.starting");
            string userDataDirectory = UserDataDirectory();
            JsonPairingStore pairingStore = new(Path.Combine(userDataDirectory, "pairing-state.json"));
            string desktopId = await new PairingManager(pairingStore).GetDesktopIdAsync();
            JsonPointerMovementSettingsStore pointerSettingsStore = new(Path.Combine(userDataDirectory, "pointer-movement-settings.json"));
            JsonMouseRepeatSettingsStore mouseRepeatSettingsStore = new(Path.Combine(userDataDirectory, "mouse-repeat-settings.json"));
            JsonCursorOverlaySettingsStore cursorOverlaySettingsStore = new(Path.Combine(userDataDirectory, "cursor-overlay-settings.json"));
            SendInputWindowsNativeInput nativeInput = new();
            WindowsDesktopInputAdapter inputAdapter = new(nativeInput, pointerSettingsStore.Load());
            cursorOverlay = new WindowsCursorOverlayNotifier(
                nativeInput,
                cursorOverlaySettingsStore,
                warn: exceptionType => RecordRuntimeDiagnostic(
                    "cursor.overlay.disabled",
                    status: "disabled",
                    reason: "render_failure",
                    message: exceptionType));
            modifierOverlay = new WindowsModifierKeyOverlayNotifier(nativeInput);
            commandExecutor = new DesktopCommandExecutor(inputAdapter, cursorOverlay, modifierOverlay: modifierOverlay);
            mouseRepeatController = new MouseRepeatController(
                commandExecutor,
                mouseRepeatSettingsStore,
                feedbackNotifier: cursorOverlay);
            PointerSpeedController pointerSpeedController = new(pointerSettingsStore, inputAdapter.SetPointerMovementSettings);
            ControlSession controlSession = new(
                new CommandAuthValidator(pairingStore),
                commandExecutor,
                new WindowsPointerProfileProvider(nativeInput, pointerSettingsStore, mouseRepeatSettingsStore),
                mouseRepeatController,
                pointerSpeedController);

            pairingApprovalManager ??= new PairingApprovalManager(pairingStore);
            RemoteControlSession remoteSession = new(
                new PairingManager(pairingStore),
                pairingApprovalManager,
                controlSession,
                () => Dispatcher.BeginInvoke(RefreshPairingApprovals));
            bluetoothFrameProcessor = new BluetoothRemoteFrameProcessor(remoteSession);
            bluetoothServer = new WindowsBluetoothGattServer(HandleBluetoothEvent);
            await bluetoothServer.StartAsync(WindowsBluetoothGattServerOptions.CreateDefault("Switchify PC", desktopId));
            RecordRuntimeDiagnostic("bluetooth.start.completed");
        }
        catch (Exception error)
        {
            bluetoothStatusTracker?.SetError(error.Message);
            RecordRuntimeDiagnostic("bluetooth.start.failed", reason: "startup_failed");
            ReportException(error, "bluetooth");
        }
    }

    private void HandleBluetoothEvent(BluetoothHelperEvent helperEvent)
    {
        if (helperEvent is BluetoothMessageEvent message)
        {
            _ = HandleBluetoothMessageSerializedAsync(message);
            return;
        }

        Dispatcher.BeginInvoke(async () =>
        {
            if (bluetoothStatusTracker is null) return;

            switch (helperEvent)
            {
                case BluetoothReadyEvent:
                    bluetoothStatusTracker.SetReady();
                    RecordRuntimeDiagnostic("bluetooth.ready", status: "ready");
                    break;
                case BluetoothUnavailableEvent unavailable:
                    bluetoothStatusTracker.SetUnavailable(unavailable.Reason);
                    RecordRuntimeDiagnostic("bluetooth.unavailable", status: "unavailable", reason: unavailable.Reason);
                    break;
                case BluetoothConnectedEvent:
                    bluetoothStatusTracker.RecordDiagnostic("transport_connected");
                    RecordRuntimeDiagnostic("bluetooth.connected", status: "connected");
                    break;
                case BluetoothDisconnectedEvent disconnected:
                    BluetoothStatus status = bluetoothStatusTracker.RemoveConnection(disconnected.ConnectionId, disconnected.Reason);
                    RecordRuntimeDiagnostic("bluetooth.disconnected", status: status.Status, reason: disconnected.Reason);
                    lock (bluetoothConnectionSync)
                    {
                        authenticatedBluetoothConnections.Remove(disconnected.ConnectionId);
                    }
                    bluetoothFrameProcessor?.RemoveConnection(disconnected.ConnectionId);
                    if (status.ConnectedClientCount == 0)
                    {
                        if (mouseRepeatController is not null)
                        {
                            await mouseRepeatController.StopAllAsync();
                        }

                        if (commandExecutor is not null)
                        {
                            await commandExecutor.ReleaseHeldInputsAsync();
                        }

                        cursorOverlay?.EndControlSession();
                        modifierOverlay?.EndControlSession();
                    }
                    break;
                case BluetoothDiagnosticEvent diagnostic:
                    bluetoothStatusTracker.RecordDiagnostic(diagnostic.Event);
                    RecordRuntimeDiagnostic("bluetooth.diagnostic", reason: diagnostic.Event);
                    break;
                case BluetoothSystemStatusEvent systemStatus:
                    bluetoothStatusTracker.SetSystemStatus(systemStatus);
                    break;
                case BluetoothErrorEvent error:
                    bluetoothStatusTracker.SetError(error.Reason);
                    RecordRuntimeDiagnostic("bluetooth.error", status: "error", reason: error.Reason);
                    break;
            }
        });
    }

    private async Task HandleBluetoothMessageSerializedAsync(BluetoothMessageEvent message)
    {
        await bluetoothMessageProcessing.WaitAsync();
        try
        {
            await HandleBluetoothMessageAsync(message).ConfigureAwait(false);
        }
        finally
        {
            bluetoothMessageProcessing.Release();
        }
    }

    private async Task HandleBluetoothMessageAsync(BluetoothMessageEvent message)
    {
        if (bluetoothFrameProcessor is null || bluetoothServer is null) return;

        BluetoothRemoteFrameResult result = await bluetoothFrameProcessor.AcceptAsync(message.ConnectionId, message.Frame);
        if (!result.MessageComplete) return;
        if (result.ErrorReason is not null)
        {
            RecordRuntimeDiagnostic("bluetooth.message.error", status: "error", reason: result.ErrorReason);
            await Dispatcher.BeginInvoke(() => bluetoothStatusTracker?.SetError(result.ErrorReason));
            return;
        }

        await SendBluetoothOutputsAsync(result.OutgoingMessages);
        if (result.AuthenticatedConnectionId is not null)
        {
            string connectionId = result.AuthenticatedConnectionId;
            bool firstAuthenticatedMessage;
            lock (bluetoothConnectionSync)
            {
                firstAuthenticatedMessage = authenticatedBluetoothConnections.Add(connectionId);
            }

            if (firstAuthenticatedMessage)
            {
                RecordRuntimeDiagnostic("bluetooth.authenticated", status: "connected");
                await Dispatcher.BeginInvoke(() => bluetoothStatusTracker?.AddConnection(connectionId));
            }
        }
        else if (result.AuthFailureReason is not null)
        {
            RecordRuntimeDiagnostic("bluetooth.auth.rejected", reason: result.AuthFailureReason);
            await Dispatcher.BeginInvoke(() => bluetoothStatusTracker?.RecordDiagnostic("unauthenticated_command_rejected"));
            string? authFailureMessage = AuthFailureMessage(result.AuthFailureReason);
            if (authFailureMessage is not null)
            {
                await Dispatcher.BeginInvoke(() => bluetoothStatusTracker?.SetError(authFailureMessage));
            }
        }

    }

    private static string? AuthFailureMessage(string reason)
    {
        return reason switch
        {
            "unknown_device" => "Bluetooth device is not approved in Switchify. Open Switchify on Android and request access.",
            "invalid_auth" => "Switchify access expired. Request access again from Android.",
            "expired_timestamp" => "Switchify command timestamp was stale. Check the device time and reconnect.",
            _ => null
        };
    }

    private async Task AcceptPairingApprovalAsync(string requestId)
    {
        if (bluetoothFrameProcessor is not null && bluetoothServer is not null)
        {
            await SendBluetoothOutputsAsync(await bluetoothFrameProcessor.AcceptPairingRequestAsync(requestId));
        }
        else
        {
            if (pairingApprovalManager is null) return;
            await pairingApprovalManager.AcceptAsync(requestId);
        }

        RefreshPairingApprovals();
        _ = RefreshSetupGuideStateAsync(allowAutoOpen: false);
    }

    private void RejectPairingApproval(string requestId)
    {
        if (bluetoothFrameProcessor is not null && bluetoothServer is not null)
        {
            _ = SendBluetoothOutputsAsync(bluetoothFrameProcessor.RejectPairingRequest(requestId));
        }
        else
        {
            pairingApprovalManager?.Reject(requestId);
        }

        RefreshPairingApprovals();
    }

    private async Task RefreshSetupGuideStateAsync(bool allowAutoOpen)
    {
        MainWindowPromptSettings settings = CreateMainWindowPromptSettingsStore().Load();
        PairingState pairingState = await new JsonPairingStore(Path.Combine(UserDataDirectory(), "pairing-state.json")).LoadAsync();
        SystemStartupSettings startupSettings = await CreateStartupService().GetSettingsAsync();
        IReadOnlyList<PendingPairingApprovalView> approvals = pairingApprovalManager?.ListPendingRequestViews() ?? [];
        BluetoothStatus bluetoothStatus = bluetoothStatusTracker?.Status ?? BluetoothStatusModel.DefaultStatus;
        bool shouldAutoOpen = allowAutoOpen && SetupGuidePrompt.ShouldAutoOpen(settings, pairingState.PairedDevices.Count > 0);

        await Dispatcher.BeginInvoke(() =>
        {
            setupGuideViewModel.SetBluetoothStatus(bluetoothStatus);
            setupGuideViewModel.SetPairingApprovals(approvals);
            setupGuideViewModel.SetHasPairedDevices(pairingState.PairedDevices.Count > 0);
            setupGuideViewModel.SetStartupSettings(startupSettings);
            if (shouldAutoOpen)
            {
                MarkSetupGuideShown();
                ShowSetupGuideWindowCore();
            }
            else
            {
                ShowDiagnosticsConsentIfNeeded();
            }
        });
    }

    private void OpenAndroidDownloadUrl()
    {
        try
        {
            Process.Start(new ProcessStartInfo(AndroidDownloadPrompt.GooglePlayUrl)
            {
                UseShellExecute = true
            });
        }
        catch
        {
            System.Windows.MessageBox.Show(
                "Could not open Google Play.",
                "Switchify for Android",
                MessageBoxButton.OK,
                MessageBoxImage.Warning);
        }
    }

    private async Task SetSetupStartWithSystemAsync(bool enabled)
    {
        SystemStartupSettings settings = await CreateStartupService().SetStartWithSystemAsync(enabled);
        await Dispatcher.BeginInvoke(() => setupGuideViewModel.SetStartupSettings(settings));
    }

    private void MarkSetupGuideShown()
    {
        IMainWindowPromptSettingsStore store = CreateMainWindowPromptSettingsStore();
        MainWindowPromptSettings settings = store.Load();
        if (settings.SetupGuideShown) return;
        store.Save(SetupGuidePrompt.MarkShown(settings));
    }

    private void CompleteSetupGuide()
    {
        IMainWindowPromptSettingsStore store = CreateMainWindowPromptSettingsStore();
        MainWindowPromptSettings settings = store.Load();
        store.Save(SetupGuidePrompt.MarkCompleted(settings));
    }

    private JsonTelemetrySettingsStore CreateTelemetrySettingsStore()
    {
        return new JsonTelemetrySettingsStore(Path.Combine(UserDataDirectory(), "telemetry-settings.json"));
    }

    private ITelemetryReporter CreateDisabledTelemetryReporter()
    {
        return new TelemetryReporter(CreateTelemetrySettingsStore(), new HttpClient(), string.Empty, CurrentVersion,
            Path.Combine(UserDataDirectory(), "telemetry-queue.json"));
    }

    private void InitializeTelemetry()
    {
        telemetrySettingsStore = CreateTelemetrySettingsStore();
        string apiKey = typeof(App).Assembly.GetCustomAttributes<AssemblyMetadataAttribute>()
            .FirstOrDefault(attribute => attribute.Key == "TimberlogsApiKey")?.Value ?? string.Empty;
        telemetryReporter = new TelemetryReporter(
            telemetrySettingsStore,
            new HttpClient(),
            apiKey,
            CurrentVersion,
            Path.Combine(UserDataDirectory(), "telemetry-queue.json"));

        DispatcherUnhandledException += (_, args) => ReportException(args.Exception, "dispatcher");
        AppDomain.CurrentDomain.UnhandledException += (_, args) =>
        {
            if (args.ExceptionObject is Exception error) ReportException(error, "appdomain");
        };
        TaskScheduler.UnobservedTaskException += (_, args) => ReportException(args.Exception, "task");
    }

    private void SetShareDiagnosticData(bool enabled)
    {
        telemetryReporter?.SetEnabled(enabled);
        setupGuideViewModel.SetShareDiagnosticData(enabled);
        if (enabled) _ = telemetryReporter?.FlushAsync();
    }

    private void ShowDiagnosticsConsentIfNeeded()
    {
        if (diagnosticsConsentShown || telemetrySettingsStore?.Load().ConsentRecorded != false || MainWindow is null) return;
        diagnosticsConsentShown = true;
        DiagnosticsConsentWindow prompt = new() { Owner = MainWindow };
        prompt.ShowDialog();
        SetShareDiagnosticData(prompt.Choice == true);
    }

    private void ReportException(Exception error, string dataset)
    {
        lock (reportedExceptionsSync)
        {
            if (!reportedExceptions.Add(error)) return;
            if (reportedExceptions.Count > 100) reportedExceptions.Clear();
        }
        _ = telemetryReporter?.ReportExceptionAsync(error, dataset);
    }

    private void StartTelemetryFlushTimer()
    {
        telemetryFlushTimer = new DispatcherTimer { Interval = TimeSpan.FromMinutes(5) };
        telemetryFlushTimer.Tick += (_, _) => _ = telemetryReporter?.FlushAsync();
        telemetryFlushTimer.Start();
    }

    private void StartPairingExpiryTimer()
    {
        pairingExpiryTimer = new DispatcherTimer
        {
            Interval = TimeSpan.FromSeconds(5)
        };
        pairingExpiryTimer.Tick += async (_, _) =>
        {
            if (bluetoothFrameProcessor is not null)
            {
                await SendBluetoothOutputsAsync(bluetoothFrameProcessor.ExpirePendingPairingRequests());
                bluetoothFrameProcessor.ClearExpiredPartials();
            }

            RefreshPairingApprovals();
        };
        pairingExpiryTimer.Start();
    }

    private async Task SendBluetoothOutputsAsync(IReadOnlyList<BluetoothRemoteFrameOutput> outputs)
    {
        if (bluetoothServer is null) return;

        foreach (BluetoothRemoteFrameOutput output in outputs)
        {
            foreach (BluetoothFrame frame in output.ResponseFrames)
            {
                await bluetoothServer.SendAsync(output.ConnectionId, frame);
            }

            if (output.CloseConnection)
            {
                bluetoothServer.Disconnect(output.ConnectionId);
            }
        }
    }

    private void RefreshPairingApprovals()
    {
        IReadOnlyList<PendingPairingApprovalView> approvals = pairingApprovalManager?.ListPendingRequestViews() ?? [];
        mainWindowViewModel.SetPairingApprovals(approvals);
        setupGuideViewModel.SetPairingApprovals(approvals);
    }

    private static string UserDataDirectory()
    {
        string appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        return Path.Combine(appData, "switchify-pc");
    }

    private static bool IsInstalledApp()
    {
        string? executablePath = Environment.ProcessPath;
        if (string.IsNullOrWhiteSpace(executablePath)) return false;
        string programFiles = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles);
        return executablePath.StartsWith(programFiles, StringComparison.OrdinalIgnoreCase);
    }

    private void UpdateMainWindowState(UpdateState state)
    {
        RecordUpdateStateDiagnostic(state);
        Dispatcher.BeginInvoke(() => mainWindowViewModel.SetUpdateState(state));
    }

    private void UpdateBluetoothState(BluetoothStatus status)
    {
        DesktopUiState desktopState = status.Status switch
        {
            "connected" => DesktopUiState.Connected,
            "ready" => DesktopUiState.WaitingForDevice,
            "starting" => DesktopUiState.Starting,
            "stopped" or "disabled" => DesktopUiState.NotRunning,
            "error" => DesktopUiState.ServerError,
            _ => DesktopUiState.Starting
        };
        Dispatcher.BeginInvoke(() => mainWindowViewModel.SetBluetoothState(desktopState, status));
        Dispatcher.BeginInvoke(() => setupGuideViewModel.SetBluetoothStatus(status));
    }

    private async Task InitializeStartupRegistrationAsync(string[] argv, bool startHidden)
    {
        await RepairStartupRegistrationAsync();
        await RecordStartupDiagnosticsAsync(argv, startHidden);
    }

    private async Task RepairStartupRegistrationAsync()
    {
        try
        {
            await CreateStartupService().RepairLegacyStartupRegistrationAsync();
        }
        catch (Exception error)
        {
            RecordRuntimeDiagnostic("startup.registration.repair.failed", reason: "repair_failed", message: error.Message);
            Console.WriteLine(string.IsNullOrWhiteSpace(error.Message)
                ? "Could not repair startup registration."
                : error.Message);
        }
    }

    private void RecordUpdateStateDiagnostic(UpdateState state)
    {
        string checkStatus = state.Info.Status.ToString();
        string downloadStatus = state.Download.Status.ToString();
        if (checkStatus == lastLoggedUpdateCheckStatus && downloadStatus == lastLoggedUpdateDownloadStatus)
        {
            return;
        }

        lastLoggedUpdateCheckStatus = checkStatus;
        lastLoggedUpdateDownloadStatus = downloadStatus;
        string? reason = state.Info.Reason?.ToString() ?? state.Download.Reason?.ToString();
        RecordRuntimeDiagnostic(
            "update.state.changed",
            status: $"{checkStatus}/{downloadStatus}",
            reason: reason);
    }

    private void RecordRuntimeDiagnostic(
        string eventName,
        string? status = null,
        string? reason = null,
        string? message = null)
    {
        JsonlDiagnostics.AppendRuntimeDiagnostic(
            Path.Combine(UserDataDirectory(), "runtime-diagnostics.jsonl"),
            new RuntimeDiagnosticEntry(
                Event: eventName,
                At: DateTimeOffset.UtcNow.ToString("O"),
                Version: CurrentVersion,
                Status: SafeDiagnosticText(status),
                Reason: SafeDiagnosticText(reason),
                Message: SafeDiagnosticText(message)));

        telemetryReporter?.RecordBreadcrumb(eventName);
        if (IsRemoteHealthEvent(eventName))
        {
            Dictionary<string, string> data = [];
            if (!string.IsNullOrWhiteSpace(status)) data["status"] = status;
            if (!string.IsNullOrWhiteSpace(reason)) data["reason"] = reason;
            _ = telemetryReporter?.ReportHealthAsync(eventName, data);
        }
    }

    private static bool IsRemoteHealthEvent(string eventName)
    {
        return eventName is "app.startup.completed" or "app.exit" or "bluetooth.ready" or
            "bluetooth.unavailable" or "bluetooth.connected" or "bluetooth.disconnected" or
            "bluetooth.error" or "bluetooth.start.failed" or "cursor.overlay.disabled" or
            "startup.registration.repair.failed" or "update.state.changed";
    }

    private static string? SafeDiagnosticText(string? value)
    {
        if (string.IsNullOrWhiteSpace(value)) return null;

        string text = value
            .Replace('\r', ' ')
            .Replace('\n', ' ')
            .Trim();
        return text.Length <= 300 ? text : text[..297] + "...";
    }

    private async Task RecordStartupDiagnosticsAsync(string[] argv, bool startHidden)
    {
        try
        {
            SystemStartupSettings startupSettings = await CreateStartupService().GetSettingsAsync();
            JsonlDiagnostics.AppendStartupDiagnostics(
                Path.Combine(UserDataDirectory(), "startup-diagnostics.jsonl"),
                new StartupDiagnosticsEntry(
                    StartedAt: DateTimeOffset.UtcNow.ToString("O"),
                    Version: CurrentVersion,
                    IsPackaged: IsInstalledApp(),
                    Platform: "win32",
                    ExecutablePath: Environment.ProcessPath ?? string.Empty,
                    Argv: argv,
                    StartHidden: startHidden,
                    StartupRegistration: JsonlDiagnostics.RegistrationFromSettings(startupSettings),
                    StartupTask: JsonlDiagnostics.TaskFromSettings(startupSettings)));
        }
        catch
        {
            // Diagnostics must not block app startup.
        }
    }
}
