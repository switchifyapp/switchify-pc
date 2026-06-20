using System.Drawing.Drawing2D;

namespace Switchify.CursorOverlay;

internal sealed class OverlayForm : Form
{
    private const int DefaultWindowSize = 128;
    private const int DefaultDurationMs = 900;
    private const int ClickPulseMs = 180;
    private const int CrosshairThickness = 2;
    private static readonly Color DefaultOverlayColor = Color.FromArgb(211, 47, 47);

    private readonly System.Windows.Forms.Timer hideTimer = new();
    private readonly System.Windows.Forms.Timer animationTimer = new();
    private readonly CrosshairLineForm horizontalCrosshair = new();
    private readonly CrosshairLineForm verticalCrosshair = new();
    private DateTime clickPulseStartedAt = DateTime.MinValue;
    private bool isClickPulse;
    private bool isDragActive;
    private int ringWindowSize = DefaultWindowSize;
    private PointF cursorCenter = new(DefaultWindowSize / 2.0f, DefaultWindowSize / 2.0f);
    private Color overlayColor = DefaultOverlayColor;

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
        Rectangle displayBounds = display.Bounds;
        ringWindowSize = size;
        overlayColor = ResolveOverlayColor(command);

        ClientSize = new Size(size, size);
        Location = new Point(
            (int)Math.Round(command.X - size / 2.0),
            (int)Math.Round(command.Y - size / 2.0));
        cursorCenter = new PointF(size / 2.0f, size / 2.0f);

        isClickPulse = string.Equals(command.Event, "click", StringComparison.OrdinalIgnoreCase);
        isDragActive = string.Equals(command.Event, "drag", StringComparison.OrdinalIgnoreCase);
        clickPulseStartedAt = isClickPulse ? DateTime.UtcNow : DateTime.MinValue;

        if (!RenderLayeredOverlay())
        {
            HideOverlay();
            return;
        }

        ApplyTopMostNoActivate();
        ShowCrosshairs(command.Crosshairs, cursor, displayBounds, overlayColor);

        hideTimer.Stop();
        if (!command.Persistent)
        {
            hideTimer.Interval = durationMs;
            hideTimer.Start();
        }
        animationTimer.Start();
    }

    internal void HideOverlay()
    {
        hideTimer.Stop();
        animationTimer.Stop();
        isClickPulse = false;
        isDragActive = false;
        horizontalCrosshair.Hide();
        verticalCrosshair.Hide();
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
            _ = RenderLayeredOverlay();
            return;
        }

        animationTimer.Stop();
    }

    private bool RenderLayeredOverlay()
    {
        using Bitmap bitmap = new(ClientSize.Width, ClientSize.Height, System.Drawing.Imaging.PixelFormat.Format32bppPArgb);
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
            float dragDotDiameter = ringDiameter * 0.22f;
            float dragDotX = centerX - dragDotDiameter / 2.0f;
            float dragDotY = centerY - dragDotDiameter / 2.0f;

            using Pen glow = new(
                Color.FromArgb(isDragActive ? 66 : 62, overlayColor.R, overlayColor.G, overlayColor.B),
                isDragActive ? outerStroke * 1.08f : outerStroke);
            using Pen ring = new(
                Color.FromArgb(250, overlayColor.R, overlayColor.G, overlayColor.B),
                isDragActive ? ringStroke + 1.0f : ringStroke);
            using SolidBrush dragDot = new(Color.FromArgb(240, overlayColor.R, overlayColor.G, overlayColor.B));

            graphics.DrawEllipse(glow, ringX, ringY, ringDiameter, ringDiameter);
            graphics.DrawEllipse(ring, ringX, ringY, ringDiameter, ringDiameter);
            if (isDragActive)
            {
                graphics.FillEllipse(dragDot, dragDotX, dragDotY, dragDotDiameter, dragDotDiameter);
            }
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

            return NativeMethods.UpdateLayeredWindow(
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

    private void ShowCrosshairs(bool enabled, Point cursor, Rectangle displayBounds, Color color)
    {
        if (!enabled)
        {
            horizontalCrosshair.Hide();
            verticalCrosshair.Hide();
            return;
        }

        horizontalCrosshair.ShowLine(
            new Rectangle(
                displayBounds.Left,
                cursor.Y - CrosshairThickness / 2,
                displayBounds.Width,
                CrosshairThickness),
            color);
        verticalCrosshair.ShowLine(
            new Rectangle(
                cursor.X - CrosshairThickness / 2,
                displayBounds.Top,
                CrosshairThickness,
                displayBounds.Height),
            color);
    }

    private static Color ResolveOverlayColor(OverlayCommand command)
    {
        if (command.Color is null)
        {
            return DefaultOverlayColor;
        }

        return Color.FromArgb(
            ClampColorChannel(command.Color.Red),
            ClampColorChannel(command.Color.Green),
            ClampColorChannel(command.Color.Blue));
    }

    private static int ClampColorChannel(int channel)
    {
        return Math.Clamp(channel, 0, 255);
    }
}

internal sealed class CrosshairLineForm : Form
{
    internal CrosshairLineForm()
    {
        AutoScaleMode = AutoScaleMode.None;
        BackColor = Color.FromArgb(211, 47, 47);
        ControlBox = false;
        FormBorderStyle = FormBorderStyle.None;
        MaximizeBox = false;
        MinimizeBox = false;
        Name = "SwitchifyCursorCrosshair";
        Opacity = 0.72;
        ShowIcon = false;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        TopMost = true;
    }

    protected override bool ShowWithoutActivation => true;

    protected override CreateParams CreateParams
    {
        get
        {
            CreateParams createParams = base.CreateParams;
            createParams.ExStyle |=
                NativeMethods.WS_EX_TRANSPARENT |
                NativeMethods.WS_EX_TOPMOST |
                NativeMethods.WS_EX_TOOLWINDOW |
                NativeMethods.WS_EX_NOACTIVATE;
            return createParams;
        }
    }

    internal void ShowLine(Rectangle bounds, Color color)
    {
        BackColor = color;
        Bounds = bounds;
        if (!Visible)
        {
            Show();
        }
        ApplyTopMostNoActivate();
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
