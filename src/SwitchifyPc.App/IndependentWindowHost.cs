using System.Windows;

namespace SwitchifyPc.App;

internal sealed class IndependentWindowHost<TWindow>
    where TWindow : Window
{
    private readonly Func<TWindow> createWindow;
    private TWindow? window;

    public IndependentWindowHost(Func<TWindow> createWindow)
    {
        this.createWindow = createWindow;
    }

    public TWindow Show()
    {
        TWindow current = window ?? CreateWindow();
        current.Owner = null;
        current.Show();
        current.WindowState = WindowState.Normal;
        current.Activate();
        return current;
    }

    private TWindow CreateWindow()
    {
        TWindow created = createWindow();
        window = created;
        created.Closed += (_, _) =>
        {
            if (ReferenceEquals(window, created))
            {
                window = null;
            }
        };
        return created;
    }
}
