using System.Windows;

namespace SwitchifyPc.App;

public partial class DiagnosticsConsentWindow : Window
{
    public DiagnosticsConsentWindow()
    {
        InitializeComponent();
        Closing += (_, _) => Choice ??= false;
    }

    public bool? Choice { get; private set; }

    private void Accept_Click(object sender, RoutedEventArgs e)
    {
        Choice = true;
        DialogResult = true;
    }

    private void Decline_Click(object sender, RoutedEventArgs e)
    {
        Choice = false;
        DialogResult = false;
    }
}
