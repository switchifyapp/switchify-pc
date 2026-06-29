using Forms = System.Windows.Forms;

namespace SwitchifyPc.App;

public sealed class NativeTrayIcon : IDisposable
{
    private readonly Forms.NotifyIcon notifyIcon;

    public NativeTrayIcon(Action showMainWindow, Action quit)
    {
        Forms.ContextMenuStrip menu = new();
        menu.Items.Add("Show Switchify PC", null, (_, _) => showMainWindow());
        menu.Items.Add(new Forms.ToolStripSeparator());
        menu.Items.Add("Quit", null, (_, _) => quit());

        notifyIcon = new Forms.NotifyIcon
        {
            Icon = System.Drawing.SystemIcons.Application,
            Text = "Switchify PC",
            ContextMenuStrip = menu,
            Visible = true
        };
        notifyIcon.DoubleClick += (_, _) => showMainWindow();
    }

    public void Dispose()
    {
        notifyIcon.Visible = false;
        notifyIcon.ContextMenuStrip?.Dispose();
        notifyIcon.Dispose();
    }
}
