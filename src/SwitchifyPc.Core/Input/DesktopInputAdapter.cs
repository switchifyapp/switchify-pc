namespace SwitchifyPc.Core.Input;

public interface IDesktopInputAdapter
{
    Task MoveMouseByAsync(double dx, double dy, CancellationToken cancellationToken = default);
    Task SetMouseButtonDownAsync(string button, bool down, CancellationToken cancellationToken = default);
    Task ClickMouseAsync(string button, CancellationToken cancellationToken = default);
    Task DoubleClickMouseAsync(string button, CancellationToken cancellationToken = default);
    Task ScrollMouseAsync(double dx, double dy, CancellationToken cancellationToken = default);
    Task PressKeyAsync(string key, CancellationToken cancellationToken = default);
    Task SetKeyDownAsync(string key, bool down, CancellationToken cancellationToken = default);
    Task PressShortcutAsync(IReadOnlyList<string> keys, CancellationToken cancellationToken = default);
    Task TypeTextAsync(string text, CancellationToken cancellationToken = default);
    Task TypeCharacterAsync(string text, CancellationToken cancellationToken = default);
    Task MediaControlAsync(string action, CancellationToken cancellationToken = default);
    Task ControlWindowAsync(string action, CancellationToken cancellationToken = default);
}

public sealed class DesktopInputException : Exception
{
    public DesktopInputException(string code, string message) : base(message)
    {
        Code = code;
    }

    public string Code { get; }
}

public enum CursorOverlayEventKind
{
    Move,
    Drag,
    Click,
    DoubleClick
}

public sealed record CursorOverlayEvent(CursorOverlayEventKind Kind, string? Button = null);

public interface ICursorOverlayNotifier
{
    void Show(CursorOverlayEvent cursorEvent);
    void Hide();
    void EndControlSession();
    void MarkControlActive();
    void SetDragActive(bool active);
}

public interface IModifierKeyOverlayNotifier
{
    void SetActiveModifiers(IReadOnlyCollection<string> activeModifiers);
    void EndControlSession();
}
