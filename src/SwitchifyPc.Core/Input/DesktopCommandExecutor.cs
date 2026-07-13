using System.Text.Json;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Core.Input;

public sealed record CommandExecutionResult(bool Ok, string? Code = null, string? Message = null)
{
    public static CommandExecutionResult Success { get; } = new(true);
    public static CommandExecutionResult Failure(string code, string message) => new(false, code, message);
}

public sealed class DesktopCommandExecutor
{
    private const int TextStreamTtlMs = 60_000;

    private readonly IDesktopInputAdapter adapter;
    private readonly ICursorOverlayNotifier? cursorOverlay;
    private readonly IModifierKeyOverlayNotifier? modifierOverlay;
    private readonly Func<double> now;
    private readonly Dictionary<string, TextInputStreamState> textInputStreams = new(StringComparer.Ordinal);
    private readonly HashSet<string> activeModifierKeys = new(StringComparer.Ordinal);
    private string? activeDragButton;

    public DesktopCommandExecutor(
        IDesktopInputAdapter adapter,
        ICursorOverlayNotifier? cursorOverlay = null,
        Func<double>? now = null,
        IModifierKeyOverlayNotifier? modifierOverlay = null)
    {
        this.adapter = adapter;
        this.cursorOverlay = cursorOverlay;
        this.modifierOverlay = modifierOverlay;
        this.now = now ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
    }

    public async Task<CommandExecutionResult> ExecuteAsync(JsonElement command, CancellationToken cancellationToken = default)
    {
        try
        {
            string type = command.GetProperty("type").GetString() ?? "";
            JsonElement payload = command.GetProperty("payload");

            if (IsMouseCommand(type))
            {
                cursorOverlay?.MarkControlActive();
            }
            else
            {
                cursorOverlay?.Hide();
            }

            return type switch
            {
                "mouse.move" => await MoveMouseAsync(payload, cancellationToken),
                "mouse.dragStart" => await StartDragAsync(payload.GetProperty("button").GetString() ?? "", cancellationToken),
                "mouse.dragEnd" => await EndDragAsync(cancellationToken),
                "mouse.click" => await ClickMouseAsync(payload.GetProperty("button").GetString() ?? "", cancellationToken),
                "mouse.doubleClick" => await DoubleClickMouseAsync(payload.GetProperty("button").GetString() ?? "", cancellationToken),
                "mouse.rightClick" => await RightClickMouseAsync(cancellationToken),
                "mouse.scroll" => await ScrollMouseAsync(payload, cancellationToken),
                "keyboard.key" => await PressKeyAsync(payload.GetProperty("key").GetString() ?? "", cancellationToken),
                "keyboard.modifierDown" => await SetModifierAsync(payload.GetProperty("key").GetString() ?? "", down: true, cancellationToken),
                "keyboard.modifierUp" => await SetModifierAsync(payload.GetProperty("key").GetString() ?? "", down: false, cancellationToken),
                "keyboard.shortcut" => await PressShortcutAsync(payload.GetProperty("keys"), cancellationToken),
                "keyboard.typeText" => await TypeTextAsync(payload.GetProperty("text").GetString() ?? "", cancellationToken),
                "keyboard.textStream.open" => OpenTextInputStream(command.GetProperty("deviceId").GetString() ?? "", payload.GetProperty("streamId").GetString() ?? ""),
                "keyboard.textStream.char" => await ExecuteTextStreamItemAsync(command, payload, () => adapter.TypeCharacterAsync(payload.GetProperty("text").GetString() ?? "", cancellationToken), "Text stream character insertion failed."),
                "keyboard.textStream.chunk" => await ExecuteTextStreamItemAsync(command, payload, () => TypeTextStreamChunkAsync(payload.GetProperty("text").GetString() ?? "", cancellationToken), "Text stream chunk insertion failed."),
                "keyboard.textStream.key" => await ExecuteTextStreamItemAsync(command, payload, () => adapter.PressKeyAsync(payload.GetProperty("key").GetString() ?? "", cancellationToken), "Text stream key insertion failed."),
                "keyboard.textStream.close" => CloseTextInputStream(command.GetProperty("deviceId").GetString() ?? "", payload.GetProperty("streamId").GetString() ?? "", payload.GetProperty("expectedCount").GetInt32()),
                "media.control" => await MediaControlAsync(payload.GetProperty("action").GetString() ?? "", cancellationToken),
                "window.control" => await WindowControlAsync(payload.GetProperty("action").GetString() ?? "", cancellationToken),
                "connection.ping" => CommandExecutionResult.Success,
                "connection.disconnecting" => CommandExecutionResult.Failure("unsupported_command", "Disconnect intent must be handled by the server."),
                "pointer.profile" => CommandExecutionResult.Failure("unsupported_command", "Pointer profile must be handled by the server."),
                _ => CommandExecutionResult.Failure("unsupported_command", "Unsupported command.")
            };
        }
        catch (DesktopInputException error)
        {
            return CommandExecutionResult.Failure(error.Code, error.Message);
        }
        catch (Exception error)
        {
            return CommandExecutionResult.Failure("adapter_failure", error.Message);
        }
    }

    public async Task ReleaseHeldMouseButtonsAsync(CancellationToken cancellationToken = default)
    {
        await ReleaseHeldInputsAsync(cancellationToken).ConfigureAwait(false);
    }

    public async Task ReleaseHeldInputsAsync(CancellationToken cancellationToken = default)
    {
        await ReleaseHeldMouseButtonAsync(cancellationToken).ConfigureAwait(false);
        await ReleaseHeldModifiersAsync(cancellationToken).ConfigureAwait(false);
    }

    public void EndControlSession()
    {
        activeDragButton = null;
        activeModifierKeys.Clear();
        cursorOverlay?.EndControlSession();
        modifierOverlay?.EndControlSession();
    }

    private async Task<CommandExecutionResult> MoveMouseAsync(JsonElement payload, CancellationToken cancellationToken)
    {
        double dx = payload.GetProperty("dx").GetDouble();
        double dy = payload.GetProperty("dy").GetDouble();
        AssertBoundedNumber(dx, ProtocolConstants.MaxPointerDelta, "dx");
        AssertBoundedNumber(dy, ProtocolConstants.MaxPointerDelta, "dy");
        await adapter.MoveMouseByAsync(dx, dy, cancellationToken);
        cursorOverlay?.Show(new CursorOverlayEvent(
            activeDragButton is null ? CursorOverlayEventKind.Move : CursorOverlayEventKind.Drag,
            activeDragButton));
        return CommandExecutionResult.Success;
    }

    private async Task<CommandExecutionResult> StartDragAsync(string button, CancellationToken cancellationToken)
    {
        if (activeDragButton == button) return CommandExecutionResult.Success;
        if (activeDragButton is not null)
        {
            await adapter.SetMouseButtonDownAsync(activeDragButton, false, cancellationToken);
            activeDragButton = null;
        }

        await adapter.SetMouseButtonDownAsync(button, true, cancellationToken);
        activeDragButton = button;
        cursorOverlay?.SetDragActive(true);
        cursorOverlay?.Show(new CursorOverlayEvent(CursorOverlayEventKind.Drag, button));
        return CommandExecutionResult.Success;
    }

    private async Task<CommandExecutionResult> EndDragAsync(CancellationToken cancellationToken)
    {
        if (activeDragButton is not null)
        {
            string button = activeDragButton;
            await adapter.SetMouseButtonDownAsync(button, false, cancellationToken);
            activeDragButton = null;
        }

        cursorOverlay?.SetDragActive(false);
        cursorOverlay?.Show(new CursorOverlayEvent(CursorOverlayEventKind.Move));
        return CommandExecutionResult.Success;
    }

    private async Task<CommandExecutionResult> ClickMouseAsync(string button, CancellationToken cancellationToken)
    {
        await ReleaseDragBeforeClickAsync(cancellationToken);
        await adapter.ClickMouseAsync(button, cancellationToken);
        cursorOverlay?.Show(new CursorOverlayEvent(CursorOverlayEventKind.Click, button));
        return CommandExecutionResult.Success;
    }

    private async Task<CommandExecutionResult> DoubleClickMouseAsync(string button, CancellationToken cancellationToken)
    {
        await ReleaseDragBeforeClickAsync(cancellationToken);
        await adapter.DoubleClickMouseAsync(button, cancellationToken);
        cursorOverlay?.Show(new CursorOverlayEvent(CursorOverlayEventKind.DoubleClick, button));
        return CommandExecutionResult.Success;
    }

    private async Task<CommandExecutionResult> RightClickMouseAsync(CancellationToken cancellationToken)
    {
        await ReleaseDragBeforeClickAsync(cancellationToken);
        await adapter.ClickMouseAsync("right", cancellationToken);
        cursorOverlay?.Show(new CursorOverlayEvent(CursorOverlayEventKind.Click, "right"));
        return CommandExecutionResult.Success;
    }

    private async Task<CommandExecutionResult> ScrollMouseAsync(JsonElement payload, CancellationToken cancellationToken)
    {
        double dx = payload.GetProperty("dx").GetDouble();
        double dy = payload.GetProperty("dy").GetDouble();
        AssertBoundedNumber(dx, ProtocolConstants.MaxScrollDelta, "dx");
        AssertBoundedNumber(dy, ProtocolConstants.MaxScrollDelta, "dy");
        await adapter.ScrollMouseAsync(dx, dy, cancellationToken);
        cursorOverlay?.Show(new CursorOverlayEvent(CursorOverlayEventKind.Scroll, Dx: dx, Dy: dy));
        return CommandExecutionResult.Success;
    }

    private async Task<CommandExecutionResult> PressKeyAsync(string key, CancellationToken cancellationToken)
    {
        await adapter.PressKeyAsync(key, cancellationToken);
        return CommandExecutionResult.Success;
    }

    private async Task<CommandExecutionResult> SetModifierAsync(string key, bool down, CancellationToken cancellationToken)
    {
        if (down)
        {
            if (activeModifierKeys.Contains(key))
            {
                return CommandExecutionResult.Success;
            }

            await adapter.SetKeyDownAsync(key, true, cancellationToken);
            activeModifierKeys.Add(key);
            UpdateModifierOverlay();
            return CommandExecutionResult.Success;
        }

        if (!activeModifierKeys.Remove(key))
        {
            return CommandExecutionResult.Success;
        }

        try
        {
            await adapter.SetKeyDownAsync(key, false, cancellationToken);
            return CommandExecutionResult.Success;
        }
        finally
        {
            UpdateModifierOverlay();
        }
    }

    private async Task<CommandExecutionResult> PressShortcutAsync(JsonElement keysElement, CancellationToken cancellationToken)
    {
        string[] keys = keysElement.EnumerateArray().Select(key => key.GetString() ?? "").ToArray();
        if (keys.Length == 0 || keys.Length > ProtocolConstants.MaxShortcutKeys)
        {
            return CommandExecutionResult.Failure("unsafe_payload", "Shortcut key count is invalid.");
        }

        await PressShortcutRespectingHeldModifiersAsync(keys, cancellationToken);
        return CommandExecutionResult.Success;
    }

    private async Task PressShortcutRespectingHeldModifiersAsync(IReadOnlyList<string> keys, CancellationToken cancellationToken)
    {
        List<string> temporaryKeys = [];
        try
        {
            foreach (string key in keys)
            {
                if (activeModifierKeys.Contains(key))
                {
                    continue;
                }

                await adapter.SetKeyDownAsync(key, true, cancellationToken).ConfigureAwait(false);
                temporaryKeys.Add(key);
            }
        }
        finally
        {
            foreach (string key in temporaryKeys.AsEnumerable().Reverse())
            {
                await adapter.SetKeyDownAsync(key, false, cancellationToken).ConfigureAwait(false);
            }
        }
    }

    private async Task<CommandExecutionResult> TypeTextAsync(string text, CancellationToken cancellationToken)
    {
        if (text.Length > ProtocolConstants.MaxTextLength)
        {
            return CommandExecutionResult.Failure("unsafe_payload", "Text payload is too long.");
        }

        await adapter.TypeTextAsync(text, cancellationToken);
        return CommandExecutionResult.Success;
    }

    private CommandExecutionResult OpenTextInputStream(string deviceId, string streamId)
    {
        ExpireTextInputStreams();
        double currentTime = now();
        textInputStreams[TextInputStreamKey(deviceId, streamId)] = new TextInputStreamState(deviceId, streamId, currentTime);
        return CommandExecutionResult.Success;
    }

    private async Task<CommandExecutionResult> ExecuteTextStreamItemAsync(JsonElement command, JsonElement payload, Func<Task> execute, string failureMessage)
    {
        string deviceId = command.GetProperty("deviceId").GetString() ?? "";
        string streamId = payload.GetProperty("streamId").GetString() ?? "";
        int seq = payload.GetProperty("seq").GetInt32();
        string key = TextInputStreamKey(deviceId, streamId);

        if (!textInputStreams.TryGetValue(key, out TextInputStreamState? stream))
        {
            return CommandExecutionResult.Failure("adapter_failure", "Text stream is not open.");
        }

        stream.UpdatedAtMs = now();
        if (seq < stream.NextSeq)
        {
            return stream.Failed
                ? CommandExecutionResult.Failure("adapter_failure", stream.ErrorMessage ?? "Text stream failed.")
                : CommandExecutionResult.Success;
        }

        if (seq > stream.NextSeq)
        {
            stream.Failed = true;
            stream.ErrorMessage = "Text stream sequence mismatch.";
            stream.NextSeq = Math.Max(stream.NextSeq, seq + 1);
            return CommandExecutionResult.Failure("adapter_failure", stream.ErrorMessage);
        }

        stream.NextSeq = seq + 1;
        if (stream.Failed)
        {
            return CommandExecutionResult.Failure("adapter_failure", stream.ErrorMessage ?? "Text stream failed.");
        }

        try
        {
            await execute();
            return CommandExecutionResult.Success;
        }
        catch
        {
            stream.Failed = true;
            stream.ErrorMessage = failureMessage;
            return CommandExecutionResult.Failure("adapter_failure", failureMessage);
        }
    }

    private CommandExecutionResult CloseTextInputStream(string deviceId, string streamId, int expectedCount)
    {
        ExpireTextInputStreams();
        string key = TextInputStreamKey(deviceId, streamId);
        if (!textInputStreams.Remove(key, out TextInputStreamState? stream))
        {
            return CommandExecutionResult.Failure("adapter_failure", "Text stream is not open.");
        }

        if (expectedCount != stream.NextSeq)
        {
            return CommandExecutionResult.Failure("adapter_failure", "Text stream did not receive every item.");
        }

        return stream.Failed
            ? CommandExecutionResult.Failure("adapter_failure", stream.ErrorMessage ?? "Text stream failed.")
            : CommandExecutionResult.Success;
    }

    private async Task TypeTextStreamChunkAsync(string text, CancellationToken cancellationToken)
    {
        await adapter.TypeTextAsync(text, cancellationToken);
    }

    private async Task<CommandExecutionResult> MediaControlAsync(string action, CancellationToken cancellationToken)
    {
        await adapter.MediaControlAsync(action, cancellationToken);
        return CommandExecutionResult.Success;
    }

    private async Task<CommandExecutionResult> WindowControlAsync(string action, CancellationToken cancellationToken)
    {
        await adapter.ControlWindowAsync(action, cancellationToken);
        return CommandExecutionResult.Success;
    }

    private void ExpireTextInputStreams()
    {
        double expiresBefore = now() - TextStreamTtlMs;
        foreach (string key in textInputStreams.Where(entry => entry.Value.UpdatedAtMs < expiresBefore).Select(entry => entry.Key).ToArray())
        {
            textInputStreams.Remove(key);
        }
    }

    private static void AssertBoundedNumber(double value, int maxAbsValue, string label)
    {
        if (!double.IsFinite(value) || Math.Abs(value) > maxAbsValue)
        {
            throw new DesktopInputException("unsafe_payload", $"{label} is outside allowed bounds.");
        }
    }

    private static bool IsMouseCommand(string type)
    {
        return type is "mouse.move" or "mouse.click" or "mouse.doubleClick" or "mouse.rightClick" or "mouse.scroll" or "mouse.dragStart" or "mouse.dragEnd";
    }

    private async Task ReleaseHeldMouseButtonAsync(CancellationToken cancellationToken)
    {
        if (activeDragButton is null) return;
        string button = activeDragButton;
        activeDragButton = null;
        await adapter.SetMouseButtonDownAsync(button, false, cancellationToken);
        cursorOverlay?.SetDragActive(false);
        cursorOverlay?.Hide();
    }

    private async Task ReleaseDragBeforeClickAsync(CancellationToken cancellationToken)
    {
        if (activeDragButton is null) return;
        string button = activeDragButton;
        activeDragButton = null;
        await adapter.SetMouseButtonDownAsync(button, false, cancellationToken);
        cursorOverlay?.SetDragActive(false);
    }

    private async Task ReleaseHeldModifiersAsync(CancellationToken cancellationToken)
    {
        foreach (string key in new[] { "Meta", "Shift", "Alt", "Ctrl" })
        {
            if (!activeModifierKeys.Remove(key)) continue;
            try
            {
                await adapter.SetKeyDownAsync(key, false, cancellationToken);
            }
            finally
            {
                UpdateModifierOverlay();
            }
        }
    }

    private void UpdateModifierOverlay()
    {
        modifierOverlay?.SetActiveModifiers(ActiveModifierLabels());
    }

    private IReadOnlyList<string> ActiveModifierLabels()
    {
        List<string> labels = [];
        foreach (string key in new[] { "Ctrl", "Alt", "Shift", "Meta" })
        {
            if (activeModifierKeys.Contains(key))
            {
                labels.Add(DisplayModifierLabel(key));
            }
        }

        return labels;
    }

    private static string DisplayModifierLabel(string key)
    {
        return key == "Meta" ? "Start" : key;
    }

    private static string TextInputStreamKey(string deviceId, string streamId)
    {
        return $"{deviceId}:{streamId}";
    }

    private sealed class TextInputStreamState
    {
        public TextInputStreamState(string deviceId, string streamId, double openedAtMs)
        {
            DeviceId = deviceId;
            StreamId = streamId;
            OpenedAtMs = openedAtMs;
            UpdatedAtMs = openedAtMs;
        }

        public string DeviceId { get; }
        public string StreamId { get; }
        public int NextSeq { get; set; }
        public bool Failed { get; set; }
        public string? ErrorMessage { get; set; }
        public double OpenedAtMs { get; }
        public double UpdatedAtMs { get; set; }
    }
}
