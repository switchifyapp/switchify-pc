using SwitchifyPc.Core.Settings;
using SwitchifyPc.Core.Startup;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;
using SwitchifyPc.Core.Pairing;

namespace SwitchifyPc.Tests;

public sealed class SettingsControllerTests
{
    [Fact]
    public async Task LoadPopulatesAllSettings()
    {
        SettingsViewModel viewModel = new();
        FakeStartupSettings startup = new(new SystemStartupSettings(
            Supported: true,
            StartWithSystem: true,
            StartsHidden: true,
            Reason: null,
            Registration: null));
        FakePointerSettings pointer = new(new PointerMovementSettings(150));
        FakeMouseRepeatSettings mouseRepeat = new(new MouseRepeatSettings(false, 500));
        FakeCursorOverlaySettings cursor = new(CursorOverlaySettingsModel.Default with
        {
            Enabled = false,
            Color = "blue"
        });
        FakeUpdates updates = new(UpdateState.CreateInitial("0.2.0") with
        {
            Info = UpdateState.CreateInitial("0.2.0").Info with
            {
                Status = UpdateCheckStatus.UpToDate,
                LatestVersion = "0.2.0"
            }
        });
        MemoryPairingStore pairing = new(new PairingState(
            "desktop-1",
            [
                new PairedDevice("device-1", "Pixel 9", "secret-token", 3_723_000, 3_724_000)
            ]));

        await new SettingsController(viewModel, startup, pointer, mouseRepeat, cursor, updates, pairing).LoadAsync();

        Assert.True(viewModel.StartWithSystem);
        Assert.Equal(150, viewModel.PointerScalePercent);
        Assert.False(viewModel.MouseRepeatEnabled);
        Assert.Equal(500, viewModel.MouseRepeatIntervalMs);
        Assert.False(viewModel.CursorOverlayEnabled);
        Assert.Equal("Blue", viewModel.CursorOverlayColor);
        Assert.Equal("Switchify PC is up to date.", viewModel.UpdateStatusMessage);
        Assert.Equal("1 paired device.", viewModel.PairedDevicesMessage);
        PairedDeviceSettingsView device = Assert.Single(viewModel.PairedDevices);
        Assert.Equal("Pixel 9", device.DeviceName);
        Assert.DoesNotContain("secret", device.ToString(), StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public async Task SetStartWithSystemDelegatesAndUpdatesViewModel()
    {
        SettingsViewModel viewModel = new();
        FakeStartupSettings startup = new(new SystemStartupSettings(false, false, true, "unpackaged"));
        SettingsController controller = CreateController(viewModel, startup: startup);

        await controller.SetStartWithSystemAsync(true);

        Assert.True(startup.LastSetValue);
        Assert.True(viewModel.StartWithSystem);
        Assert.Equal("Switchify PC will start hidden when you sign in.", viewModel.StartWithSystemMessage);
    }

    [Fact]
    public void SetPointerScaleSavesNormalizedValueAndUpdatesDerivedPercentages()
    {
        SettingsViewModel viewModel = new();
        FakePointerSettings pointer = new(PointerMovementSettingsModel.Default);
        SettingsController controller = CreateController(viewModel, pointer: pointer);

        PointerMovementSettings saved = controller.SetPointerScalePercent(212);

        Assert.Equal(200, saved.ScalePercent);
        Assert.Equal(new PointerMovementSettings(200), pointer.Saved);
        Assert.Equal(200, viewModel.PointerScalePercent);
        Assert.Equal("9%", viewModel.PointerSmall);
        Assert.Equal("24%", viewModel.PointerMedium);
        Assert.Equal("50%", viewModel.PointerLarge);
    }

    [Fact]
    public async Task SetMouseRepeatSettingsSavesNormalizedValuesAndUpdatesViewModel()
    {
        SettingsViewModel viewModel = new();
        FakeMouseRepeatSettings mouseRepeat = new(MouseRepeatSettingsModel.Default);
        SettingsController controller = CreateController(viewModel, mouseRepeat: mouseRepeat);
        await controller.LoadAsync();

        MouseRepeatSettings saved = controller.SetMouseRepeatEnabled(false);
        saved = controller.SetMouseRepeatIntervalMs(47);

        Assert.False(saved.Enabled);
        Assert.Equal(100, saved.IntervalMs);
        Assert.Equal(saved, mouseRepeat.Saved);
        Assert.False(viewModel.MouseRepeatEnabled);
        Assert.Equal("0.1 s", viewModel.MouseRepeatInterval);
    }

    [Fact]
    public async Task CursorOverlayActionsSaveNormalizedSettingsAndUpdateViewModel()
    {
        SettingsViewModel viewModel = new();
        FakeCursorOverlaySettings cursor = new(CursorOverlaySettingsModel.Default);
        SettingsController controller = CreateController(viewModel, cursor: cursor);
        await controller.LoadAsync();

        CursorOverlaySettings saved = controller.SetCursorOverlayColor("blue");
        saved = controller.SetCursorOverlayVisibility("whileControlling");
        saved = controller.SetCursorOverlaySize("not-a-size");
        saved = controller.SetCursorOverlayEnabled(false);
        saved = controller.SetCursorOverlayCrosshairs(true);

        Assert.Equal(CursorOverlaySettingsModel.Default.Size, saved.Size);
        Assert.Equal("blue", saved.Color);
        Assert.False(saved.Enabled);
        Assert.True(saved.Crosshairs);
        Assert.Equal("whileControlling", saved.Visibility);
        Assert.Equal(saved, cursor.Saved);
        Assert.Equal("Blue", viewModel.CursorOverlayColor);
        Assert.Equal("While controlling", viewModel.CursorOverlayVisibility);
        Assert.False(viewModel.CursorOverlayEnabled);
        Assert.True(viewModel.CursorOverlayCrosshairs);
    }

    [Fact]
    public async Task UpdateActionsDelegateAndRefreshViewModel()
    {
        SettingsViewModel viewModel = new();
        UpdateState initial = UpdateState.CreateInitial("0.2.0");
        UpdateState available = initial with
        {
            Info = initial.Info with
            {
                Status = UpdateCheckStatus.UpdateAvailable,
                LatestVersion = "0.2.1"
            }
        };
        UpdateState downloaded = available with
        {
            Download = new UpdateDownloadProgress(UpdateDownloadStatus.Downloaded, 100, 100, 100)
        };
        FakeUpdates updates = new(initial)
        {
            CheckResult = available,
            DownloadResult = downloaded,
            InstallResult = UpdateInstallResult.Success()
        };
        SettingsController controller = CreateController(viewModel, updates: updates);

        await controller.CheckForUpdatesAsync();
        Assert.Equal("Update available: v0.2.1.", viewModel.UpdateStatusMessage);
        Assert.True(viewModel.CanDownloadUpdate);

        await controller.DownloadUpdateAsync();
        Assert.Equal("Update downloaded and ready to install.", viewModel.UpdateDownloadMessage);
        Assert.True(viewModel.CanInstallUpdate);

        UpdateInstallResult result = await controller.InstallDownloadedUpdateAsync();
        Assert.True(result.Ok);
        Assert.Equal(1, updates.InstallCalls);
        Assert.True(viewModel.CanInstallUpdate);
    }

    [Fact]
    public async Task ForgetPairedDeviceRemovesPersistedDeviceAndRefreshesViewModel()
    {
        SettingsViewModel viewModel = new();
        MemoryPairingStore pairingStore = new(new PairingState(
            "desktop-1",
            [
                new PairedDevice("device-1", "Pixel 9", "secret-token-1", 3_723_000, null),
                new PairedDevice("device-2", "Galaxy Tab", "secret-token-2", 3_724_000, null)
            ]));
        SettingsController controller = CreateController(viewModel, pairingStore: pairingStore);
        await controller.LoadAsync();

        bool removed = await controller.ForgetPairedDeviceAsync("device-1");

        Assert.True(removed);
        PairingState state = await pairingStore.LoadAsync();
        Assert.DoesNotContain(state.PairedDevices, device => device.DeviceId == "device-1");
        PairedDevice remaining = Assert.Single(state.PairedDevices);
        Assert.Equal("device-2", remaining.DeviceId);
        PairedDeviceSettingsView view = Assert.Single(viewModel.PairedDevices);
        Assert.Equal("Galaxy Tab", view.DeviceName);
        Assert.Equal("1 paired device.", viewModel.PairedDevicesMessage);
    }

    [Fact]
    public async Task ForgetPairedDeviceReturnsFalseForMissingDevice()
    {
        SettingsViewModel viewModel = new();
        MemoryPairingStore pairingStore = new(new PairingState(
            "desktop-1",
            [
                new PairedDevice("device-1", "Pixel 9", "secret-token", 3_723_000, null)
            ]));
        SettingsController controller = CreateController(viewModel, pairingStore: pairingStore);
        await controller.LoadAsync();

        bool removed = await controller.ForgetPairedDeviceAsync("missing-device");

        Assert.False(removed);
        Assert.Single((await pairingStore.LoadAsync()).PairedDevices);
        Assert.Single(viewModel.PairedDevices);
    }

    [Fact]
    public void ApplyUpdateStateUpdatesViewModelFromPushEvents()
    {
        SettingsViewModel viewModel = new();
        SettingsController controller = CreateController(viewModel);

        controller.ApplyUpdateState(UpdateState.CreateInitial("0.2.0") with
        {
            Info = UpdateState.CreateInitial("0.2.0").Info with
            {
                Status = UpdateCheckStatus.CheckFailed,
                Reason = UpdateFailureReason.NetworkError
            }
        });

        Assert.Equal("Could not check for updates.", viewModel.UpdateStatusMessage);
    }

    private static SettingsController CreateController(
        SettingsViewModel viewModel,
        FakeStartupSettings? startup = null,
        FakePointerSettings? pointer = null,
        FakeMouseRepeatSettings? mouseRepeat = null,
        FakeCursorOverlaySettings? cursor = null,
        FakeUpdates? updates = null,
        IPairingStore? pairingStore = null)
    {
        return new SettingsController(
            viewModel,
            startup ?? new FakeStartupSettings(new SystemStartupSettings(false, false, true, "unpackaged")),
            pointer ?? new FakePointerSettings(PointerMovementSettingsModel.Default),
            mouseRepeat ?? new FakeMouseRepeatSettings(MouseRepeatSettingsModel.Default),
            cursor ?? new FakeCursorOverlaySettings(CursorOverlaySettingsModel.Default),
            updates ?? new FakeUpdates(UpdateState.CreateInitial("0.2.0")),
            pairingStore ?? new MemoryPairingStore());
    }

    private sealed class FakeStartupSettings(SystemStartupSettings initial) : ISystemStartupSettingsService
    {
        private SystemStartupSettings settings = initial;

        public bool? LastSetValue { get; private set; }

        public Task<SystemStartupSettings> GetSettingsAsync() => Task.FromResult(settings);

        public Task<SystemStartupSettings> SetStartWithSystemAsync(bool enabled)
        {
            LastSetValue = enabled;
            settings = new SystemStartupSettings(
                Supported: true,
                StartWithSystem: enabled,
                StartsHidden: true,
                Reason: null,
                Registration: null);
            return Task.FromResult(settings);
        }
    }

    private sealed class FakePointerSettings(PointerMovementSettings initial) : IPointerMovementSettingsStore
    {
        private PointerMovementSettings settings = initial;

        public PointerMovementSettings? Saved { get; private set; }

        public PointerMovementSettings Load() => settings;

        public PointerMovementSettings Save(PointerMovementSettings next)
        {
            settings = PointerMovementSettingsModel.Normalize(next);
            Saved = settings;
            return settings;
        }
    }

    private sealed class FakeMouseRepeatSettings(MouseRepeatSettings initial) : IMouseRepeatSettingsStore
    {
        private MouseRepeatSettings settings = initial;

        public MouseRepeatSettings? Saved { get; private set; }

        public MouseRepeatSettings Load() => settings;

        public MouseRepeatSettings Save(MouseRepeatSettings next)
        {
            settings = MouseRepeatSettingsModel.Normalize(next);
            Saved = settings;
            return settings;
        }
    }

    private sealed class FakeCursorOverlaySettings(CursorOverlaySettings initial) : ICursorOverlaySettingsStore
    {
        private CursorOverlaySettings settings = initial;

        public CursorOverlaySettings? Saved { get; private set; }

        public CursorOverlaySettings Load() => settings;

        public CursorOverlaySettings Save(CursorOverlaySettings next)
        {
            settings = CursorOverlaySettingsModel.Normalize(next);
            Saved = settings;
            return settings;
        }
    }

    private sealed class FakeUpdates(UpdateState initial) : IUpdateSettingsService
    {
        private UpdateState state = initial;

        public UpdateState CheckResult { get; init; } = initial;

        public UpdateState DownloadResult { get; init; } = initial;

        public UpdateInstallResult InstallResult { get; init; } = UpdateInstallResult.Failure(UpdateInstallFailureReason.NotDownloaded);

        public int InstallCalls { get; private set; }

        public UpdateState GetState() => state;

        public Task<UpdateState> CheckForUpdatesAsync(CancellationToken cancellationToken = default)
        {
            state = CheckResult;
            return Task.FromResult(state);
        }

        public Task<UpdateState> DownloadUpdateAsync(CancellationToken cancellationToken = default)
        {
            state = DownloadResult;
            return Task.FromResult(state);
        }

        public Task<UpdateInstallResult> InstallDownloadedUpdateAsync(CancellationToken cancellationToken = default)
        {
            InstallCalls++;
            return Task.FromResult(InstallResult);
        }
    }
}
