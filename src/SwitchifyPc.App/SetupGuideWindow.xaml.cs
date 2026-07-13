using System.Windows;
using SwitchifyPc.Core.Ui;
using WpfCheckBox = System.Windows.Controls.CheckBox;

namespace SwitchifyPc.App;

public partial class SetupGuideWindow : Window
{
    private readonly SetupGuideViewModel viewModel;
    private readonly Action? openAndroidDownload;
    private readonly Func<string, Task>? acceptPairingApproval;
    private readonly Action<string>? rejectPairingApproval;
    private readonly Func<bool, Task>? setStartWithSystem;
    private readonly Action? finish;
    private readonly Action? skip;
    private readonly Action<bool>? setShareDiagnosticData;

    public SetupGuideWindow(
        SetupGuideViewModel? viewModel = null,
        Action? openAndroidDownload = null,
        Func<string, Task>? acceptPairingApproval = null,
        Action<string>? rejectPairingApproval = null,
        Func<bool, Task>? setStartWithSystem = null,
        Action? finish = null,
        Action? skip = null,
        Action<bool>? setShareDiagnosticData = null)
    {
        this.viewModel = viewModel ?? new SetupGuideViewModel();
        this.openAndroidDownload = openAndroidDownload;
        this.acceptPairingApproval = acceptPairingApproval;
        this.rejectPairingApproval = rejectPairingApproval;
        this.setStartWithSystem = setStartWithSystem;
        this.finish = finish;
        this.skip = skip;
        this.setShareDiagnosticData = setShareDiagnosticData;
        InitializeComponent();
        DataContext = this.viewModel;
    }

    private void ShareDiagnostics_Click(object sender, RoutedEventArgs e)
    {
        viewModel.SetShareDiagnosticData(true);
        setShareDiagnosticData?.Invoke(true);
    }

    private void DeclineDiagnostics_Click(object sender, RoutedEventArgs e)
    {
        viewModel.SetShareDiagnosticData(false);
        setShareDiagnosticData?.Invoke(false);
    }

    private void Back_Click(object sender, RoutedEventArgs e)
    {
        viewModel.MoveBack();
    }

    private void Next_Click(object sender, RoutedEventArgs e)
    {
        if (viewModel.IsFinalStep)
        {
            finish?.Invoke();
            Hide();
            return;
        }

        viewModel.MoveNext();
    }

    private void Skip_Click(object sender, RoutedEventArgs e)
    {
        skip?.Invoke();
        Hide();
    }

    private void OpenAndroidDownload_Click(object sender, RoutedEventArgs e)
    {
        openAndroidDownload?.Invoke();
    }

    private async void AcceptPairing_Click(object sender, RoutedEventArgs e)
    {
        if (sender is FrameworkElement { Tag: string requestId } && acceptPairingApproval is not null)
        {
            await RunActionAsync(() => acceptPairingApproval(requestId), "That pairing request could not be accepted.");
        }
    }

    private void RejectPairing_Click(object sender, RoutedEventArgs e)
    {
        if (sender is FrameworkElement { Tag: string requestId })
        {
            rejectPairingApproval?.Invoke(requestId);
        }
    }

    private async void StartWithSystem_Click(object sender, RoutedEventArgs e)
    {
        if (sender is WpfCheckBox checkBox && setStartWithSystem is not null)
        {
            checkBox.IsEnabled = false;
            try
            {
                await setStartWithSystem(checkBox.IsChecked == true);
            }
            catch
            {
                System.Windows.MessageBox.Show(
                    "Start with system could not be changed.",
                    "Switchify PC setup",
                    MessageBoxButton.OK,
                    MessageBoxImage.Warning);
            }
            finally
            {
                checkBox.IsChecked = viewModel.StartWithSystem;
                checkBox.IsEnabled = viewModel.StartWithSystemSupported;
            }
        }
    }

    private static async Task RunActionAsync(Func<Task> action, string failureMessage)
    {
        try
        {
            await action();
        }
        catch
        {
            System.Windows.MessageBox.Show(
                failureMessage,
                "Switchify PC setup",
                MessageBoxButton.OK,
                MessageBoxImage.Warning);
        }
    }
}
