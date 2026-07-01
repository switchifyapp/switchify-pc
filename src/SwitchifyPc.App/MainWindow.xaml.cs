using System.Windows;
using SwitchifyPc.Core.Ui;

namespace SwitchifyPc.App;

public partial class MainWindow : Window
{
    private readonly Action? openSettings;
    private readonly Action? openUpdates;
    private readonly Action? disconnectDevices;
    private readonly Func<string, Task>? acceptPairingApproval;
    private readonly Action<string>? rejectPairingApproval;
    private readonly Action? openAndroidDownload;
    private readonly Action? dismissAndroidDownload;

    public MainWindow(
        MainWindowViewModel? viewModel = null,
        Action? openSettings = null,
        Action? openUpdates = null,
        Action? disconnectDevices = null,
        Func<string, Task>? acceptPairingApproval = null,
        Action<string>? rejectPairingApproval = null,
        Action? openAndroidDownload = null,
        Action? dismissAndroidDownload = null)
    {
        this.openSettings = openSettings;
        this.openUpdates = openUpdates;
        this.disconnectDevices = disconnectDevices;
        this.acceptPairingApproval = acceptPairingApproval;
        this.rejectPairingApproval = rejectPairingApproval;
        this.openAndroidDownload = openAndroidDownload;
        this.dismissAndroidDownload = dismissAndroidDownload;
        InitializeComponent();
        DataContext = viewModel ?? new MainWindowViewModel();
    }

    private void OpenSettings_Click(object sender, RoutedEventArgs e)
    {
        openSettings?.Invoke();
    }

    private void OpenUpdates_Click(object sender, RoutedEventArgs e)
    {
        openUpdates?.Invoke();
    }

    private void Disconnect_Click(object sender, RoutedEventArgs e)
    {
        disconnectDevices?.Invoke();
    }

    private async void AcceptPairing_Click(object sender, RoutedEventArgs e)
    {
        if (sender is FrameworkElement { Tag: string requestId } && acceptPairingApproval is not null)
        {
            await acceptPairingApproval(requestId);
        }
    }

    private void RejectPairing_Click(object sender, RoutedEventArgs e)
    {
        if (sender is FrameworkElement { Tag: string requestId })
        {
            rejectPairingApproval?.Invoke(requestId);
        }
    }

    private void OpenAndroidDownload_Click(object sender, RoutedEventArgs e)
    {
        openAndroidDownload?.Invoke();
    }

    private void DismissAndroidDownload_Click(object sender, RoutedEventArgs e)
    {
        dismissAndroidDownload?.Invoke();
    }
}
