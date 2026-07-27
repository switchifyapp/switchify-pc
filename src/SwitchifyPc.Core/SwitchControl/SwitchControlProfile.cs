namespace SwitchifyPc.Core.SwitchControl;

public enum SwitchControlProviderKind
{
    Grid3,
    Mapped
}

public enum SwitchBindingType
{
    None,
    Key,
    MouseButton,
    Shortcut,
    MouseClick,
    Scroll,
    Media
}

public enum SwitchBindingBehavior
{
    Unassigned,
    Stateful,
    Pulse
}

public sealed record SwitchControlBinding(
    int SwitchId,
    SwitchBindingType Type,
    string? Value = null,
    IReadOnlyList<string>? Keys = null,
    int ClickCount = 1)
{
    public SwitchBindingBehavior Behavior => Type switch
    {
        SwitchBindingType.Key or SwitchBindingType.MouseButton => SwitchBindingBehavior.Stateful,
        SwitchBindingType.None => SwitchBindingBehavior.Unassigned,
        _ => SwitchBindingBehavior.Pulse
    };

    public string Label => Type switch
    {
        SwitchBindingType.None => "Unassigned",
        SwitchBindingType.Key => Value ?? "Key",
        SwitchBindingType.MouseButton => $"{Title(Value)} mouse button",
        SwitchBindingType.Shortcut => string.Join(" + ", Keys ?? []),
        SwitchBindingType.MouseClick => $"{(ClickCount == 2 ? "Double " : "")}{Title(Value)} click",
        SwitchBindingType.Scroll => $"Scroll {Value}",
        SwitchBindingType.Media => MediaLabel(Value),
        _ => "Unassigned"
    };

    private static string Title(string? value) =>
        string.IsNullOrWhiteSpace(value) ? "" : char.ToUpperInvariant(value[0]) + value[1..];

    private static string MediaLabel(string? value) => value switch
    {
        "playPause" => "Play / pause",
        "nextTrack" => "Next track",
        "previousTrack" => "Previous track",
        "volumeUp" => "Volume up",
        "volumeDown" => "Volume down",
        "mute" => "Mute",
        _ => "Media"
    };
}

public sealed record SwitchControlProfile(
    string Id,
    int Version,
    string Name,
    SwitchControlProviderKind Kind,
    IReadOnlyList<SwitchControlBinding> Bindings,
    bool IsBuiltIn = false);

public sealed record SwitchControlBindingSummary(
    int SwitchId,
    string Label,
    string Behavior);

public sealed record SwitchControlProfileSummary(
    string Id,
    int Version,
    string Name,
    string Kind,
    IReadOnlyList<SwitchControlBindingSummary> Bindings);

public sealed record SwitchControlProfileCatalog(
    int CatalogRevision,
    IReadOnlyList<SwitchControlProfileSummary> Profiles);

public static class SwitchControlProfiles
{
    public const string Grid3Id = "builtin.grid3";
    public const string GenericKeyboardId = "builtin.keyboard";
    public const int MaximumCustomProfiles = 32;

    public static IReadOnlyList<SwitchControlProfile> BuiltIns { get; } =
    [
        new(
            Grid3Id,
            1,
            "Grid 3",
            SwitchControlProviderKind.Grid3,
            Enumerable.Range(1, 8)
                .Select(id => new SwitchControlBinding(id, SwitchBindingType.None, $"Grid switch {id}"))
                .ToArray(),
            true),
        new(
            GenericKeyboardId,
            1,
            "Generic keyboard",
            SwitchControlProviderKind.Mapped,
            [
                new(1, SwitchBindingType.Key, "Space"),
                new(2, SwitchBindingType.Key, "Enter"),
                .. Enumerable.Range(3, 6).Select(id => new SwitchControlBinding(id, SwitchBindingType.None))
            ],
            true)
    ];

    public static SwitchControlProfileSummary Summarize(SwitchControlProfile profile) =>
        new(
            profile.Id,
            profile.Version,
            profile.Name,
            profile.Kind == SwitchControlProviderKind.Grid3 ? "grid3" : "mapped",
            profile.Bindings.Select(binding => new SwitchControlBindingSummary(
                binding.SwitchId,
                profile.Kind == SwitchControlProviderKind.Grid3 ? $"Grid switch {binding.SwitchId}" : binding.Label,
                profile.Kind == SwitchControlProviderKind.Grid3 ? "stateful" : BehaviorValue(binding.Behavior))).ToArray());

    private static string BehaviorValue(SwitchBindingBehavior behavior) => behavior switch
    {
        SwitchBindingBehavior.Stateful => "stateful",
        SwitchBindingBehavior.Pulse => "pulse",
        _ => "unassigned"
    };
}
