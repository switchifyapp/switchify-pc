using System.Windows;
using SwitchifyPc.Core.Ui;

namespace SwitchifyPc.App;

public partial class MainWindow : Window
{
    public MainWindow(MainWindowViewModel? viewModel = null)
    {
        InitializeComponent();
        DataContext = viewModel ?? new MainWindowViewModel();
    }
}
