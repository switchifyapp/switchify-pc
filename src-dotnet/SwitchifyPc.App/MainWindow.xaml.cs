using System.Windows;
using SwitchifyPc.Core.Ui;

namespace SwitchifyPc.App;

public partial class MainWindow : Window
{
    private readonly Action? openUpdates;
    private readonly Func<string, Task>? acceptPairingApproval;
    private readonly Action<string>? rejectPairingApproval;

    public MainWindow(
        MainWindowViewModel? viewModel = null,
        Action? openUpdates = null,
        Func<string, Task>? acceptPairingApproval = null,
        Action<string>? rejectPairingApproval = null)
    {
        this.openUpdates = openUpdates;
        this.acceptPairingApproval = acceptPairingApproval;
        this.rejectPairingApproval = rejectPairingApproval;
        InitializeComponent();
        DataContext = viewModel ?? new MainWindowViewModel();
    }

    private void OpenUpdates_Click(object sender, RoutedEventArgs e)
    {
        openUpdates?.Invoke();
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
