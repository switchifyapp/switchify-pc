using SwitchifyPc.Core.Input;

namespace SwitchifyPc.Core.SwitchControl;

public interface ISwitchOutputSession
{
    Task ApplyEdgeAsync(int switchId, bool pressed, CancellationToken token);
    Task SynchronizeAsync(IReadOnlySet<int> pressedSwitchIds, CancellationToken token);
    Task StopAsync(CancellationToken token);
}

public interface ISwitchOutputSessionFactory
{
    ISwitchOutputSession Create(SwitchControlProfile profile);
}

public sealed class SwitchOutputSessionFactory(
    IDesktopInputAdapter desktopInput,
    IGridSwitchBroadcaster? gridSwitchBroadcaster) : ISwitchOutputSessionFactory
{
    public ISwitchOutputSession Create(SwitchControlProfile profile) => profile.Kind switch
    {
        SwitchControlProviderKind.Grid3 when gridSwitchBroadcaster is not null =>
            new Grid3SwitchOutputSession(gridSwitchBroadcaster),
        SwitchControlProviderKind.Grid3 =>
            throw new DesktopInputException("output_unavailable", "Grid 3 output is unavailable."),
        _ => new MappedDesktopSwitchOutputSession(desktopInput, profile.Bindings)
    };
}

public sealed class Grid3SwitchOutputSession(IGridSwitchBroadcaster broadcaster) : ISwitchOutputSession
{
    private readonly HashSet<int> pressed = [];

    public async Task ApplyEdgeAsync(int switchId, bool isPressed, CancellationToken token)
    {
        if (isPressed ? pressed.Contains(switchId) : !pressed.Contains(switchId))
        {
            return;
        }

        await broadcaster.SetSwitchStateAsync(switchId, isPressed, token).ConfigureAwait(false);
        if (isPressed) pressed.Add(switchId); else pressed.Remove(switchId);
    }

    public async Task SynchronizeAsync(IReadOnlySet<int> pressedSwitchIds, CancellationToken token)
    {
        foreach (int switchId in pressed.Except(pressedSwitchIds).Order())
        {
            await ApplyEdgeAsync(switchId, false, token).ConfigureAwait(false);
        }
        foreach (int switchId in pressedSwitchIds.Except(pressed).Order())
        {
            await ApplyEdgeAsync(switchId, true, token).ConfigureAwait(false);
        }
    }

    public Task StopAsync(CancellationToken token) => SynchronizeAsync(new HashSet<int>(), token);
}

public sealed class MappedDesktopSwitchOutputSession : ISwitchOutputSession
{
    private readonly IDesktopInputAdapter input;
    private readonly IReadOnlyDictionary<int, SwitchControlBinding> bindings;
    private readonly HashSet<int> pressedSources = [];
    private readonly Dictionary<string, int> heldOutputCounts = new(StringComparer.OrdinalIgnoreCase);
    private readonly List<string> outputAcquisitionOrder = [];

    public MappedDesktopSwitchOutputSession(
        IDesktopInputAdapter input,
        IReadOnlyList<SwitchControlBinding> bindings)
    {
        this.input = input;
        this.bindings = bindings.ToDictionary(binding => binding.SwitchId);
    }

    public async Task ApplyEdgeAsync(int switchId, bool pressed, CancellationToken token)
    {
        bool changed = pressed ? pressedSources.Add(switchId) : pressedSources.Remove(switchId);
        if (!changed || !bindings.TryGetValue(switchId, out SwitchControlBinding? binding))
        {
            return;
        }

        try
        {
            if (binding.Behavior == SwitchBindingBehavior.Stateful)
            {
                await SetStatefulBindingAsync(binding, pressed, token).ConfigureAwait(false);
            }
            else if (pressed && binding.Behavior == SwitchBindingBehavior.Pulse)
            {
                await ExecutePulseAsync(binding, token).ConfigureAwait(false);
            }
        }
        catch
        {
            if (pressed) pressedSources.Remove(switchId); else pressedSources.Add(switchId);
            throw;
        }
    }

    public async Task SynchronizeAsync(IReadOnlySet<int> pressedSwitchIds, CancellationToken token)
    {
        foreach (int switchId in pressedSources.Except(pressedSwitchIds).Order().ToArray())
        {
            if (bindings.TryGetValue(switchId, out SwitchControlBinding? binding) &&
                binding.Behavior == SwitchBindingBehavior.Stateful)
            {
                await ApplyEdgeAsync(switchId, false, token).ConfigureAwait(false);
            }
            else
            {
                pressedSources.Remove(switchId);
            }
        }

        foreach (int switchId in pressedSwitchIds.Except(pressedSources).Order())
        {
            if (bindings.TryGetValue(switchId, out SwitchControlBinding? binding) &&
                binding.Behavior == SwitchBindingBehavior.Stateful)
            {
                await ApplyEdgeAsync(switchId, true, token).ConfigureAwait(false);
            }
            else
            {
                pressedSources.Add(switchId);
            }
        }
    }

    public async Task StopAsync(CancellationToken token)
    {
        string[] outputs = outputAcquisitionOrder
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .OrderBy(output => IsModifier(output) ? 1 : 0)
            .ThenByDescending(output => outputAcquisitionOrder.LastIndexOf(output))
            .ToArray();
        Exception? firstError = null;
        foreach (string output in outputs)
        {
            try
            {
                await SetOutputAsync(output, false, token).ConfigureAwait(false);
                heldOutputCounts.Remove(output);
            }
            catch (Exception error)
            {
                firstError ??= error;
            }
        }
        pressedSources.Clear();
        outputAcquisitionOrder.Clear();
        if (firstError is not null) throw firstError;
    }

    private async Task SetStatefulBindingAsync(SwitchControlBinding binding, bool pressed, CancellationToken token)
    {
        string output = binding.Type == SwitchBindingType.Key
            ? $"key:{binding.Value}"
            : $"mouse:{binding.Value}";
        int count = heldOutputCounts.GetValueOrDefault(output);
        int next = pressed ? count + 1 : Math.Max(0, count - 1);
        if (count == 0 && next == 1)
        {
            await SetOutputAsync(output, true, token).ConfigureAwait(false);
            outputAcquisitionOrder.Add(output);
        }
        else if (count == 1 && next == 0)
        {
            await SetOutputAsync(output, false, token).ConfigureAwait(false);
        }

        if (next == 0) heldOutputCounts.Remove(output); else heldOutputCounts[output] = next;
    }

    private async Task ExecutePulseAsync(SwitchControlBinding binding, CancellationToken token)
    {
        switch (binding.Type)
        {
            case SwitchBindingType.Shortcut:
                await ExecuteShortcutAsync(binding.Keys ?? [], token).ConfigureAwait(false);
                break;
            case SwitchBindingType.MouseClick when binding.ClickCount == 2:
                await input.DoubleClickMouseAsync(binding.Value ?? "", token).ConfigureAwait(false);
                break;
            case SwitchBindingType.MouseClick:
                await input.ClickMouseAsync(binding.Value ?? "", token).ConfigureAwait(false);
                break;
            case SwitchBindingType.Scroll:
                (double dx, double dy) = binding.Value switch
                {
                    "up" => (0, 1),
                    "down" => (0, -1),
                    "left" => (-1, 0),
                    _ => (1, 0)
                };
                await input.ScrollMouseAsync(dx, dy, token).ConfigureAwait(false);
                break;
            case SwitchBindingType.Media:
                await input.MediaControlAsync(binding.Value ?? "", token).ConfigureAwait(false);
                break;
        }
    }

    private async Task ExecuteShortcutAsync(IReadOnlyList<string> keys, CancellationToken token)
    {
        List<string> temporary = [];
        try
        {
            foreach (string key in keys)
            {
                string output = $"key:{key}";
                if (heldOutputCounts.ContainsKey(output)) continue;
                await input.SetKeyDownAsync(key, true, token).ConfigureAwait(false);
                temporary.Add(key);
            }
        }
        finally
        {
            foreach (string key in temporary.AsEnumerable().Reverse())
            {
                await input.SetKeyDownAsync(key, false, token).ConfigureAwait(false);
            }
        }
    }

    private Task SetOutputAsync(string output, bool down, CancellationToken token)
    {
        string value = output[(output.IndexOf(':') + 1)..];
        return output.StartsWith("mouse:", StringComparison.Ordinal)
            ? input.SetMouseButtonDownAsync(value, down, token)
            : input.SetKeyDownAsync(value, down, token);
    }

    private static bool IsModifier(string output) =>
        output is "key:Ctrl" or "key:Alt" or "key:Shift" or "key:Meta";
}
