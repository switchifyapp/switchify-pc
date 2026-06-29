using System.Windows;
using SwitchifyPc.Core.Ui;

namespace SwitchifyPc.App;

public partial class SettingsWindow : Window
{
    public SettingsWindow(SettingsViewModel? viewModel = null)
    {
        InitializeComponent();
        DataContext = viewModel ?? new SettingsViewModel();
    }
}
