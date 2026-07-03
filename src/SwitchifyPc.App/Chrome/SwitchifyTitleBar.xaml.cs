using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using WpfButton = System.Windows.Controls.Button;
using WpfUserControl = System.Windows.Controls.UserControl;

namespace SwitchifyPc.App.Chrome;

public partial class SwitchifyTitleBar : WpfUserControl
{
    public static readonly DependencyProperty TitleTextProperty = DependencyProperty.Register(
        nameof(TitleText),
        typeof(string),
        typeof(SwitchifyTitleBar),
        new PropertyMetadata(string.Empty));

    public static readonly DependencyProperty ShowMinimizeButtonProperty = DependencyProperty.Register(
        nameof(ShowMinimizeButton),
        typeof(bool),
        typeof(SwitchifyTitleBar),
        new PropertyMetadata(true));

    public SwitchifyTitleBar()
    {
        InitializeComponent();
    }

    public string TitleText
    {
        get => (string)GetValue(TitleTextProperty);
        set => SetValue(TitleTextProperty, value);
    }

    public bool ShowMinimizeButton
    {
        get => (bool)GetValue(ShowMinimizeButtonProperty);
        set => SetValue(ShowMinimizeButtonProperty, value);
    }

    private void TitleBar_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (e.ClickCount != 1 || IsInsideButton(e.OriginalSource as DependencyObject))
        {
            return;
        }

        Window? window = Window.GetWindow(this);
        if (window is null) return;

        try
        {
            window.DragMove();
        }
        catch (InvalidOperationException)
        {
        }
    }

    private void MinimizeButton_Click(object sender, RoutedEventArgs e)
    {
        Window? window = Window.GetWindow(this);
        if (window is not null)
        {
            window.WindowState = WindowState.Minimized;
        }
    }

    private void CloseButton_Click(object sender, RoutedEventArgs e)
    {
        Window.GetWindow(this)?.Close();
    }

    private static bool IsInsideButton(DependencyObject? source)
    {
        while (source is not null)
        {
            if (source is WpfButton)
            {
                return true;
            }

            source = VisualTreeHelper.GetParent(source);
        }

        return false;
    }
}
