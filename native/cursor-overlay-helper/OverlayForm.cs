using System.Drawing.Drawing2D;

namespace Switchify.CursorOverlay;

internal sealed class OverlayForm : Form
{
    private const int DefaultWindowSize = 128;
    private const int DefaultDurationMs = 900;
    private const int ClickPulseMs = 180;

    private readonly System.Windows.Forms.Timer hideTimer = new();
    private readonly System.Windows.Forms.Timer animationTimer = new();
    private DateTime clickPulseStartedAt = DateTime.MinValue;
    private bool isClickPulse;
    private bool crosshairsEnabled;
    private int ringWindowSize = DefaultWindowSize;
    private PointF cursorCenter = new(DefaultWindowSize / 2.0f, DefaultWindowSize / 2.0f);

    internal OverlayForm()
    {
        AutoScaleMode = AutoScaleMode.None;
        BackColor = Color.Black;
        ClientSize = new Size(DefaultWindowSize, DefaultWindowSize);
        ControlBox = false;
        FormBorderStyle = FormBorderStyle.None;
        MaximizeBox = false;
        MinimizeBox = false;
        Name = "SwitchifyCursorOverlay";
        ShowIcon = false;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        TopMost = true;

        hideTimer.Tick += (_, _) => HideOverlay();
        animationTimer.Interval = 16;
        animationTimer.Tick += (_, _) => TickAnimation();
    }

    protected override bool ShowWithoutActivation => true;

    protected override CreateParams CreateParams
    {
        get
        {
            CreateParams createParams = base.CreateParams;
            createParams.ExStyle |=
                NativeMethods.WS_EX_LAYERED |
                NativeMethods.WS_EX_TRANSPARENT |
                NativeMethods.WS_EX_TOPMOST |
                NativeMethods.WS_EX_TOOLWINDOW |
                NativeMethods.WS_EX_NOACTIVATE;
            return createParams;
        }
    }

    internal void ShowOverlay(OverlayCommand command)
    {
        int size = command.Size > 0 ? command.Size : DefaultWindowSize;
        int durationMs = command.DurationMs > 0 ? command.DurationMs : DefaultDurationMs;
        Point cursor = new((int)Math.Round(command.X), (int)Math.Round(command.Y));
        Screen display = Screen.FromPoint(cursor);
        Rectangle bounds = display.Bounds;
        crosshairsEnabled = command.Crosshairs;
        ringWindowSize = size;

        if (crosshairsEnabled)
        {
            ClientSize = bounds.Size;
            Location = bounds.Location;
            cursorCenter = new PointF(cursor.X - bounds.X, cursor.Y - bounds.Y);
        }
        else
        {
            ClientSize = new Size(size, size);
            Location = new Point(
                (int)Math.Round(command.X - size / 2.0),
                (int)Math.Round(command.Y - size / 2.0));
            cursorCenter = new PointF(size / 2.0f, size / 2.0f);
        }

        isClickPulse = string.Equals(command.Event, "click", StringComparison.OrdinalIgnoreCase);
        clickPulseStartedAt = isClickPulse ? DateTime.UtcNow : DateTime.MinValue;

        hideTimer.Stop();
        if (!command.Persistent)
        {
            hideTimer.Interval = durationMs;
            hideTimer.Start();
        }
        animationTimer.Start();

        if (!Visible)
        {
            Show();
        }

        RenderLayeredOverlay();
        ApplyTopMostNoActivate();
    }

    internal void HideOverlay()
    {
        hideTimer.Stop();
        animationTimer.Stop();
        isClickPulse = false;
        crosshairsEnabled = false;
        Hide();
    }

    private float ResolvePulseScale()
    {
        if (!isClickPulse)
        {
            return 1.0f;
        }

        double elapsedMs = (DateTime.UtcNow - clickPulseStartedAt).TotalMilliseconds;
        if (elapsedMs >= ClickPulseMs)
        {
            isClickPulse = false;
            return 1.0f;
        }

        double progress = Math.Clamp(elapsedMs / ClickPulseMs, 0.0, 1.0);
        return (float)(0.82 + progress * 0.36);
    }

    private void TickAnimation()
    {
        if (isClickPulse)
        {
            RenderLayeredOverlay();
            return;
        }

        animationTimer.Stop();
    }

    private void RenderLayeredOverlay()
    {
        using Bitmap bitmap = new(ClientSize.Width, ClientSize.Height, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
        using (Graphics graphics = Graphics.FromImage(bitmap))
        {
            graphics.SmoothingMode = SmoothingMode.AntiAlias;
            graphics.Clear(Color.Transparent);

            float scale = ResolvePulseScale();
            float ringDiameter = ringWindowSize * 0.5625f * scale;
            float ringStroke = Math.Max(4.0f, ringWindowSize * 0.039f);
            float outerStroke = Math.Max(18.0f, ringWindowSize * 0.1875f) * scale;
            float centerX = cursorCenter.X;
            float centerY = cursorCenter.Y;
            float ringX = centerX - ringDiameter / 2.0f;
            float ringY = centerY - ringDiameter / 2.0f;

            using Pen glow = new(Color.FromArgb(62, 132, 255, 145), outerStroke);
            using Pen ring = new(Color.FromArgb(250, 132, 255, 145), ringStroke);
            using Pen crosshair = new(Color.FromArgb(184, 132, 255, 145), 2.0f);

            if (crosshairsEnabled)
            {
                graphics.DrawLine(crosshair, 0.0f, centerY, ClientSize.Width, centerY);
                graphics.DrawLine(crosshair, centerX, 0.0f, centerX, ClientSize.Height);
            }

            graphics.DrawEllipse(glow, ringX, ringY, ringDiameter, ringDiameter);
            graphics.DrawEllipse(ring, ringX, ringY, ringDiameter, ringDiameter);
        }

        IntPtr screenDc = NativeMethods.GetDC(IntPtr.Zero);
        IntPtr memoryDc = NativeMethods.CreateCompatibleDC(screenDc);
        IntPtr bitmapHandle = bitmap.GetHbitmap(Color.FromArgb(0));
        IntPtr oldBitmap = NativeMethods.SelectObject(memoryDc, bitmapHandle);

        try
        {
            NativeMethods.Point destination = new(Location.X, Location.Y);
            NativeMethods.Size size = new(ClientSize.Width, ClientSize.Height);
            NativeMethods.Point source = new(0, 0);
            NativeMethods.BlendFunction blend = new()
            {
                BlendOp = NativeMethods.AC_SRC_OVER,
                BlendFlags = 0,
                SourceConstantAlpha = 255,
                AlphaFormat = NativeMethods.AC_SRC_ALPHA
            };

            NativeMethods.UpdateLayeredWindow(
                Handle,
                screenDc,
                ref destination,
                ref size,
                memoryDc,
                ref source,
                0,
                ref blend,
                NativeMethods.ULW_ALPHA);
        }
        finally
        {
            NativeMethods.SelectObject(memoryDc, oldBitmap);
            NativeMethods.DeleteObject(bitmapHandle);
            NativeMethods.DeleteDC(memoryDc);
            NativeMethods.ReleaseDC(IntPtr.Zero, screenDc);
        }
    }

    private void ApplyTopMostNoActivate()
    {
        NativeMethods.SetWindowPos(
            Handle,
            NativeMethods.HWND_TOPMOST,
            0,
            0,
            0,
            0,
            NativeMethods.SWP_NOMOVE |
            NativeMethods.SWP_NOSIZE |
            NativeMethods.SWP_NOACTIVATE |
            NativeMethods.SWP_SHOWWINDOW);
    }
}
