using System.Drawing.Drawing2D;

namespace Switchify.CursorOverlay;

internal sealed class OverlayForm : Form
{
    private const int DefaultWindowSize = 72;
    private const int DefaultDurationMs = 900;
    private const int ClickPulseMs = 180;

    private readonly System.Windows.Forms.Timer hideTimer = new();
    private readonly System.Windows.Forms.Timer animationTimer = new();
    private DateTime clickPulseStartedAt = DateTime.MinValue;
    private bool isClickPulse;

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

        ClientSize = new Size(size, size);
        Location = new Point(
            (int)Math.Round(command.X - size / 2.0),
            (int)Math.Round(command.Y - size / 2.0));

        isClickPulse = string.Equals(command.Event, "click", StringComparison.OrdinalIgnoreCase);
        clickPulseStartedAt = isClickPulse ? DateTime.UtcNow : DateTime.MinValue;

        hideTimer.Stop();
        hideTimer.Interval = durationMs;
        hideTimer.Start();
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
            float ringDiameter = 34.0f * scale;
            float ringStroke = 3.0f;
            float outerStroke = 13.0f * scale;
            float centerX = ClientSize.Width / 2.0f;
            float centerY = ClientSize.Height / 2.0f;
            float ringX = centerX - ringDiameter / 2.0f;
            float ringY = centerY - ringDiameter / 2.0f;

            using Pen glow = new(Color.FromArgb(62, 132, 255, 145), outerStroke);
            using Pen ring = new(Color.FromArgb(250, 132, 255, 145), ringStroke);

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
