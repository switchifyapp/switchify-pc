using System.Windows;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;
using WpfMessageBox = System.Windows.MessageBox;

namespace SwitchifyPc.App;

public partial class MainWindow : Window
{
    private readonly Action? openSettings;
    private readonly Action? openSetupGuide;
    private readonly Func<Task<UpdateApplyResult>>? installUpdate;
    private readonly Action? disconnectDevices;
    private readonly Func<string, Task>? acceptPairingApproval;
    private readonly Action<string>? rejectPairingApproval;
    private bool isInstallingUpdate;

    public MainWindow(
        MainWindowViewModel? viewModel = null,
        Action? openSettings = null,
        Action? openSetupGuide = null,
        Func<Task<UpdateApplyResult>>? installUpdate = null,
        Action? disconnectDevices = null,
        Func<string, Task>? acceptPairingApproval = null,
        Action<string>? rejectPairingApproval = null)
    {
        this.openSettings = openSettings;
        this.openSetupGuide = openSetupGuide;
        this.installUpdate = installUpdate;
        this.disconnectDevices = disconnectDevices;
        this.acceptPairingApproval = acceptPairingApproval;
        this.rejectPairingApproval = rejectPairingApproval;
        InitializeComponent();
        DataContext = viewModel ?? new MainWindowViewModel();
    }

    private void OpenSettings_Click(object sender, RoutedEventArgs e)
    {
        openSettings?.Invoke();
    }

    private void OpenSetupGuide_Click(object sender, RoutedEventArgs e)
    {
        openSetupGuide?.Invoke();
    }

    private async void InstallUpdate_Click(object sender, RoutedEventArgs e)
    {
        if (installUpdate is null || isInstallingUpdate) return;

        isInstallingUpdate = true;
        try
        {
            UpdateApplyResult result = await installUpdate();
            if (!result.Ok)
            {
                WpfMessageBox.Show(
                    UpdateUiCopy.ApplyFailureMessage(result),
                    "Update installer",
                    MessageBoxButton.OK,
                    MessageBoxImage.Warning);
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch
        {
            WpfMessageBox.Show(
                "The update could not be installed.",
                "Update installer",
                MessageBoxButton.OK,
                MessageBoxImage.Warning);
        }
        finally
        {
            isInstallingUpdate = false;
        }
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

}
