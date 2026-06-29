using System.Windows;
using SwitchifyPc.Core.Ui;

namespace SwitchifyPc.App;

public partial class MainWindow : Window
{
    private readonly Action? openUpdates;

    public MainWindow(MainWindowViewModel? viewModel = null, Action? openUpdates = null)
    {
        this.openUpdates = openUpdates;
        InitializeComponent();
        DataContext = viewModel ?? new MainWindowViewModel();
    }

    private void OpenUpdates_Click(object sender, RoutedEventArgs e)
    {
        openUpdates?.Invoke();
    }
}
