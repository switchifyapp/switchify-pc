using System.Windows;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;
using WpfButton = System.Windows.Controls.Button;
using WpfCheckBox = System.Windows.Controls.CheckBox;
using WpfComboBox = System.Windows.Controls.ComboBox;
using WpfMessageBox = System.Windows.MessageBox;
using WpfSelectionChangedEventArgs = System.Windows.Controls.SelectionChangedEventArgs;

namespace SwitchifyPc.App;

public partial class SettingsWindow : Window
{
    private readonly SettingsController? controller;
    private bool isLoaded;
    private bool isApplyingSettings;

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
            }
            finally
            {
                isApplyingSettings = false;
            }
        }, "Settings could not be loaded.");
    }

    private async void StartWithSystem_Click(object sender, RoutedEventArgs e)
    {
        if (controller is null || isApplyingSettings || sender is not WpfCheckBox checkBox) return;
        await RunActionAsync(
            () => controller.SetStartWithSystemAsync(checkBox.IsChecked == true),
            "Start with system could not be changed.");
    }

    private void PointerScale25_Click(object sender, RoutedEventArgs e) => controller?.SetPointerScalePercent(25);

    private void PointerScale50_Click(object sender, RoutedEventArgs e) => controller?.SetPointerScalePercent(50);

    private void PointerScale75_Click(object sender, RoutedEventArgs e) => controller?.SetPointerScalePercent(75);

    private void PointerScale100_Click(object sender, RoutedEventArgs e) => controller?.SetPointerScalePercent(100);

    private void SectionNav_Checked(object sender, RoutedEventArgs e)
    {
        if (sender is not FrameworkElement { Tag: string section }) return;

        GeneralPanel.Visibility = section == "general" ? Visibility.Visible : Visibility.Collapsed;
        PointerPanel.Visibility = section == "pointer" ? Visibility.Visible : Visibility.Collapsed;
        CursorPanel.Visibility = section == "cursor" ? Visibility.Visible : Visibility.Collapsed;
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
        if (!isApplyingSettings && controller is not null && sender is WpfCheckBox checkBox)
        {
            controller.SetCursorOverlayEnabled(checkBox.IsChecked == true);
        }
    }

    private void CursorOverlayCrosshairs_Click(object sender, RoutedEventArgs e)
    {
        if (!isApplyingSettings && controller is not null && sender is WpfCheckBox checkBox)
        {
            controller.SetCursorOverlayCrosshairs(checkBox.IsChecked == true);
        }
    }

    private void CursorOverlaySize_SelectionChanged(object sender, WpfSelectionChangedEventArgs e)
    {
        if (!isApplyingSettings) SaveSelectedValue(sender, value => controller?.SetCursorOverlaySize(value));
    }

    private void CursorOverlayVisibility_SelectionChanged(object sender, WpfSelectionChangedEventArgs e)
    {
        if (!isApplyingSettings) SaveSelectedValue(sender, value => controller?.SetCursorOverlayVisibility(value));
    }

    private void CursorOverlayColor_SelectionChanged(object sender, WpfSelectionChangedEventArgs e)
    {
        if (!isApplyingSettings) SaveSelectedValue(sender, value => controller?.SetCursorOverlayColor(value));
    }

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
        MessageBoxResult confirmation = WpfMessageBox.Show(
            "Open the downloaded Switchify PC installer?\n\nThe installer will open in Windows and may ask you to close Switchify PC before continuing. If you rely on Switchify to control this computer, make sure you have another way to complete the installer before continuing.",
            "Install update?",
            MessageBoxButton.OKCancel,
            MessageBoxImage.Warning,
            MessageBoxResult.Cancel);
        if (confirmation != MessageBoxResult.OK) return;

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

    private static void SaveSelectedValue(object sender, Action<string> save)
    {
        if (sender is WpfComboBox { SelectedValue: string value })
        {
            save(value);
        }
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
