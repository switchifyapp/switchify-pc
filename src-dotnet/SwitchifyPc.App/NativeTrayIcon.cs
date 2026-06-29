using Forms = System.Windows.Forms;

namespace SwitchifyPc.App;

public sealed class NativeTrayIcon : IDisposable
{
    private readonly Forms.NotifyIcon notifyIcon;
    private readonly Forms.ToolStripMenuItem statusItem;
    private readonly Forms.ToolStripMenuItem disconnectItem;

    public NativeTrayIcon(
        Action showMainWindow,
        Action showSettingsWindow,
        Func<string> statusText,
        Func<bool> canDisconnect,
        Action disconnectDevices,
        Action quit)
    {
        Forms.ContextMenuStrip menu = new();
        menu.Items.Add("Show Switchify PC", null, (_, _) => showMainWindow());
        menu.Items.Add("Open settings", null, (_, _) => showSettingsWindow());
        menu.Items.Add(new Forms.ToolStripSeparator());
        statusItem = new Forms.ToolStripMenuItem("Status unavailable")
        {
            Enabled = false
        };
        disconnectItem = new Forms.ToolStripMenuItem("Disconnect devices", null, (_, _) => disconnectDevices());
        menu.Items.Add(statusItem);
        menu.Items.Add(disconnectItem);
        menu.Items.Add(new Forms.ToolStripSeparator());
        menu.Items.Add("Quit", null, (_, _) => quit());
        menu.Opening += (_, _) =>
        {
            statusItem.Text = statusText();
            disconnectItem.Enabled = canDisconnect();
        };

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
