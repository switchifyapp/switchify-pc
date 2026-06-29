using System.Windows;
using System.Windows.Threading;
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
using SwitchifyPc.Windows.AppLifecycle;
using SwitchifyPc.Windows.Bluetooth;
using SwitchifyPc.Windows.CursorOverlay;
using SwitchifyPc.Windows.Input;
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
    private MainWindowViewModel mainWindowViewModel = new();
    private UpdateService? updateService;
    private PairingApprovalManager? pairingApprovalManager;
    private BluetoothStatusTracker? bluetoothStatusTracker;
    private WindowsBluetoothGattServer? bluetoothServer;
    private BluetoothRemoteFrameProcessor? bluetoothFrameProcessor;
    private WindowsCursorOverlayNotifier? cursorOverlay;
    private DispatcherTimer? pairingExpiryTimer;
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
            if (decision.ExistingInstanceAction == ExistingInstanceAction.ShowMainWindow)
            {
                using WindowsExistingInstanceSignal signal = new();
                signal.SignalShowMainWindow();
            }

            Shutdown();
            return;
        }

        existingInstanceSignal = new WindowsExistingInstanceSignal();
        existingInstanceSignal.Start(() => Dispatcher.BeginInvoke(ShowMainWindow));
        updateService = CreateUpdateService();
        pairingApprovalManager = CreatePairingApprovalManager();
        bluetoothStatusTracker = new BluetoothStatusTracker(onStatusChanged: UpdateBluetoothState);
        RefreshPairingApprovals();
        updateService.StartAutomaticUpdateChecks();
        StartPairingExpiryTimer();
        _ = StartBluetoothAsync();
        _ = RecordStartupDiagnosticsAsync(e.Args, launchOptions.StartHidden);
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
    }

    protected override void OnExit(ExitEventArgs e)
    {
        singleInstance?.Stop();
        singleInstance = null;
        existingInstanceSignal?.Dispose();
        existingInstanceSignal = null;
        updateService?.StopAutomaticUpdateChecks();
        updateService = null;
        pairingExpiryTimer?.Stop();
        pairingExpiryTimer = null;
        bluetoothServer?.Dispose();
        bluetoothServer = null;
        cursorOverlay?.Dispose();
        cursorOverlay = null;
        bluetoothFrameProcessor = null;
        bluetoothStatusTracker = null;
        pairingApprovalManager = null;
        trayIcon?.Dispose();
        trayIcon = null;
        settingsWindow = null;
        base.OnExit(e);
    }

    private void ShowMainWindow()
    {
        Window window = MainWindow ?? CreateMainWindow();
        MainWindow = window;

        window.Show();
        window.WindowState = WindowState.Normal;
        window.Activate();
    }

    private Window CreateMainWindow()
    {
        mainWindowViewModel.SetUpdateState(updateService?.GetState() ?? UpdateState.CreateInitial(CurrentVersion));
        RefreshPairingApprovals();
        SwitchifyPc.App.MainWindow window = new(
            mainWindowViewModel,
            ShowSettingsWindow,
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
        settingsWindow.Show();
        settingsWindow.WindowState = WindowState.Normal;
        settingsWindow.Activate();
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

    private SettingsController CreateSettingsController()
    {
        SettingsViewModel viewModel = new();
        string userDataDirectory = UserDataDirectory();
        return new SettingsController(
            viewModel,
            CreateStartupService(),
            new JsonPointerMovementSettingsStore(Path.Combine(userDataDirectory, "pointer-movement-settings.json")),
            new JsonCursorOverlaySettingsStore(Path.Combine(userDataDirectory, "cursor-overlay-settings.json")),
            updateService ?? CreateUpdateService());
    }

    private SystemStartupService CreateStartupService()
    {
        return new SystemStartupService(
            platform: "win32",
            isPackaged: IsInstalledApp(),
            executablePath: Environment.ProcessPath ?? string.Empty,
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

    private async Task StartBluetoothAsync()
    {
        try
        {
            bluetoothStatusTracker?.SetStarting();
            string userDataDirectory = UserDataDirectory();
            JsonPairingStore pairingStore = new(Path.Combine(userDataDirectory, "pairing-state.json"));
            string desktopId = await new PairingManager(pairingStore).GetDesktopIdAsync();
            JsonPointerMovementSettingsStore pointerSettingsStore = new(Path.Combine(userDataDirectory, "pointer-movement-settings.json"));
            JsonCursorOverlaySettingsStore cursorOverlaySettingsStore = new(Path.Combine(userDataDirectory, "cursor-overlay-settings.json"));
            SendInputWindowsNativeInput nativeInput = new();
            WindowsDesktopInputAdapter inputAdapter = new(nativeInput, pointerSettingsStore.Load());
            cursorOverlay = new WindowsCursorOverlayNotifier(nativeInput, cursorOverlaySettingsStore);
            ControlSession controlSession = new(
                new CommandAuthValidator(pairingStore),
                new DesktopCommandExecutor(inputAdapter, cursorOverlay),
                new WindowsPointerProfileProvider(nativeInput, pointerSettingsStore));

            pairingApprovalManager ??= new PairingApprovalManager(pairingStore);
            RemoteControlSession remoteSession = new(
                new PairingManager(pairingStore),
                pairingApprovalManager,
                controlSession,
                () => Dispatcher.BeginInvoke(RefreshPairingApprovals));
            bluetoothFrameProcessor = new BluetoothRemoteFrameProcessor(remoteSession);
            bluetoothServer = new WindowsBluetoothGattServer(HandleBluetoothEvent);
            await bluetoothServer.StartAsync(WindowsBluetoothGattServerOptions.CreateDefault("Switchify PC", desktopId));
        }
        catch (Exception error)
        {
            bluetoothStatusTracker?.SetError(error.Message);
        }
    }

    private void HandleBluetoothEvent(BluetoothHelperEvent helperEvent)
    {
        Dispatcher.BeginInvoke(async () =>
        {
            if (bluetoothStatusTracker is null) return;

            switch (helperEvent)
            {
                case BluetoothReadyEvent:
                    bluetoothStatusTracker.SetReady();
                    break;
                case BluetoothUnavailableEvent unavailable:
                    bluetoothStatusTracker.SetUnavailable(unavailable.Reason);
                    break;
                case BluetoothConnectedEvent connected:
                    bluetoothStatusTracker.AddConnection(connected.ConnectionId);
                    break;
                case BluetoothDisconnectedEvent disconnected:
                    bluetoothStatusTracker.RemoveConnection(disconnected.ConnectionId, disconnected.Reason);
                    bluetoothFrameProcessor?.RemoveConnection(disconnected.ConnectionId);
                    break;
                case BluetoothDiagnosticEvent diagnostic:
                    bluetoothStatusTracker.RecordDiagnostic(diagnostic.Event);
                    break;
                case BluetoothSystemStatusEvent systemStatus:
                    bluetoothStatusTracker.SetSystemStatus(systemStatus);
                    break;
                case BluetoothErrorEvent error:
                    bluetoothStatusTracker.SetError(error.Reason);
                    break;
                case BluetoothMessageEvent message:
                    await HandleBluetoothMessageAsync(message);
                    break;
            }
        });
    }

    private async Task HandleBluetoothMessageAsync(BluetoothMessageEvent message)
    {
        if (bluetoothFrameProcessor is null || bluetoothServer is null) return;

        BluetoothRemoteFrameResult result = await bluetoothFrameProcessor.AcceptAsync(message.ConnectionId, message.Frame);
        if (!result.MessageComplete) return;
        if (result.ErrorReason is not null)
        {
            bluetoothStatusTracker?.SetError(result.ErrorReason);
            return;
        }

        await SendBluetoothOutputsAsync(result.OutgoingMessages);
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
        mainWindowViewModel.SetPairingApprovals(
            pairingApprovalManager?.ListPendingRequestViews() ?? []);
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
                    StartupRegistration: JsonlDiagnostics.RegistrationFromSettings(startupSettings)));
        }
        catch
        {
            // Diagnostics must not block app startup.
        }
    }
}
