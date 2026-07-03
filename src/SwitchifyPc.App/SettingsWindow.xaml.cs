using System.Windows;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;
using WpfButton = System.Windows.Controls.Button;
using WpfCheckBox = System.Windows.Controls.CheckBox;
using WpfMessageBox = System.Windows.MessageBox;

namespace SwitchifyPc.App;

public partial class SettingsWindow : Window
{
    private readonly SettingsController? controller;
    private bool isLoaded;
    private bool isApplyingSettings;
    private bool settingsLoaded;

    public SettingsWindow(SettingsController controller) : this(controller.ViewModel)
    {
        this.controller = controller;
    }

    public SettingsWindow(SettingsViewModel? viewModel = null)
    {
        InitializeComponent();
        DataContext = viewModel ?? new SettingsViewModel();
        Loaded += SettingsWindow_Loaded;
    }

    private async void SettingsWindow_Loaded(object sender, RoutedEventArgs e)
    {
        if (isLoaded) return;
        isLoaded = true;
        if (controller is null) return;

        await RunActionAsync(async () =>
        {
            isApplyingSettings = true;
            try
            {
                await controller.LoadAsync();
                settingsLoaded = true;
            }
            finally
            {
                isApplyingSettings = false;
            }
        }, "Settings could not be loaded.");
    }

    private async void StartWithSystem_Click(object sender, RoutedEventArgs e)
    {
        if (controller is null || isApplyingSettings || !settingsLoaded || sender is not WpfCheckBox checkBox) return;
        await RunActionAsync(
            () => controller.SetStartWithSystemAsync(checkBox.IsChecked == true),
            "Start with system could not be changed.");
    }

    private void PointerScale25_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetPointerScalePercent(25));

    private void PointerScale50_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetPointerScalePercent(50));

    private void PointerScale75_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetPointerScalePercent(75));

    private void PointerScale100_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetPointerScalePercent(100));

    private void MouseRepeatEnabled_Click(object sender, RoutedEventArgs e)
    {
        if (!isApplyingSettings && settingsLoaded && controller is not null && sender is WpfCheckBox checkBox)
        {
            controller.SetMouseRepeatEnabled(checkBox.IsChecked == true);
        }
    }

    private void MouseRepeatMoveInterval100_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetMouseRepeatMoveIntervalMs(100));

    private void MouseRepeatMoveInterval250_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetMouseRepeatMoveIntervalMs(250));

    private void MouseRepeatMoveInterval500_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetMouseRepeatMoveIntervalMs(500));

    private void MouseRepeatMoveInterval1000_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetMouseRepeatMoveIntervalMs(1000));

    private void MouseRepeatScrollInterval100_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetMouseRepeatScrollIntervalMs(100));

    private void MouseRepeatScrollInterval250_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetMouseRepeatScrollIntervalMs(250));

    private void MouseRepeatScrollInterval500_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetMouseRepeatScrollIntervalMs(500));

    private void MouseRepeatScrollInterval1000_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetMouseRepeatScrollIntervalMs(1000));

    private void SectionNav_Checked(object sender, RoutedEventArgs e)
    {
        if (sender is not FrameworkElement { Tag: string section }) return;

        GeneralPanel.Visibility = section == "general" ? Visibility.Visible : Visibility.Collapsed;
        PointerPanel.Visibility = section == "pointer" ? Visibility.Visible : Visibility.Collapsed;
        UpdatesPanel.Visibility = section == "updates" ? Visibility.Visible : Visibility.Collapsed;
    }

    private async void ForgetPairedDevice_Click(object sender, RoutedEventArgs e)
    {
        if (controller is null || sender is not WpfButton { Tag: string deviceId }) return;

        await RunActionAsync(async () =>
        {
            bool removed = await controller.ForgetPairedDeviceAsync(deviceId);
            if (!removed)
            {
                WpfMessageBox.Show(
                    "That saved device is no longer available.",
                    "Saved devices",
                    MessageBoxButton.OK,
                    MessageBoxImage.Warning);
            }
        }, "That saved device could not be forgotten.");
    }

    private void CursorOverlayEnabled_Click(object sender, RoutedEventArgs e)
    {
        if (!isApplyingSettings && settingsLoaded && controller is not null && sender is WpfCheckBox checkBox)
        {
            controller.SetCursorOverlayEnabled(checkBox.IsChecked == true);
        }
    }

    private void CursorOverlayCrosshairs_Click(object sender, RoutedEventArgs e)
    {
        if (!isApplyingSettings && settingsLoaded && controller is not null && sender is WpfCheckBox checkBox)
        {
            controller.SetCursorOverlayCrosshairs(checkBox.IsChecked == true);
        }
    }

    private void CursorOverlaySizeSmall_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetCursorOverlaySize("small"));

    private void CursorOverlaySizeMedium_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetCursorOverlaySize("medium"));

    private void CursorOverlaySizeLarge_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetCursorOverlaySize("large"));

    private void CursorOverlayVisibilityOnInput_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetCursorOverlayVisibility("onInput"));

    private void CursorOverlayVisibilityWhileControlling_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetCursorOverlayVisibility("whileControlling"));

    private void CursorOverlayColorRed_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetCursorOverlayColor("red"));

    private void CursorOverlayColorGreen_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetCursorOverlayColor("green"));

    private void CursorOverlayColorBlue_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetCursorOverlayColor("blue"));

    private void CursorOverlayColorYellow_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetCursorOverlayColor("yellow"));

    private void CursorOverlayColorWhite_Checked(object sender, RoutedEventArgs e) => SaveIfReady(() => controller!.SetCursorOverlayColor("white"));

    private async void CheckForUpdates_Click(object sender, RoutedEventArgs e)
    {
        if (controller is null) return;
        await RunActionAsync(
            () => controller.CheckForUpdatesAsync(),
            "Updates could not be checked.");
    }

    private async void DownloadUpdate_Click(object sender, RoutedEventArgs e)
    {
        if (controller is null) return;
        await RunActionAsync(
            () => controller.DownloadUpdateAsync(),
            "The update could not be downloaded.");
    }

    private async void InstallUpdate_Click(object sender, RoutedEventArgs e)
    {
        if (controller is null) return;

        await RunActionAsync(async () =>
        {
            UpdateInstallResult result = await controller.InstallDownloadedUpdateAsync();
            if (!result.Ok)
            {
                WpfMessageBox.Show(
                    SettingsViewModel.InstallMessage(result.Reason),
                    "Update installer",
                    MessageBoxButton.OK,
                    MessageBoxImage.Warning);
            }
        }, "The update installer could not be opened.");
    }

    private void SaveIfReady(Action save)
    {
        if (controller is null || isApplyingSettings || !settingsLoaded) return;
        save();
    }

    private async Task RunActionAsync(Func<Task> action, string failureMessage)
    {
        try
        {
            await action();
        }
        catch
        {
            WpfMessageBox.Show(failureMessage, "Switchify PC settings", MessageBoxButton.OK, MessageBoxImage.Warning);
        }
    }

}
