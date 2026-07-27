using System.Text.Json;
using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Core.SwitchControl;

public interface ISwitchControlProfileStore
{
    IReadOnlyList<SwitchControlProfile> Load();
    IReadOnlyList<SwitchControlProfile> Save(IReadOnlyList<SwitchControlProfile> customProfiles);
}

public sealed class JsonSwitchControlProfileStore : ISwitchControlProfileStore
{
    private const int SchemaVersion = 1;
    private readonly string filePath;
    private readonly Action<string> warn;

    public JsonSwitchControlProfileStore(string filePath, Action<string>? warn = null)
    {
        this.filePath = filePath;
        this.warn = warn ?? Console.WriteLine;
    }

    public IReadOnlyList<SwitchControlProfile> Load()
    {
        if (!File.Exists(filePath))
        {
            return SwitchControlProfiles.BuiltIns;
        }

        try
        {
            StoredProfiles stored = JsonSerializer.Deserialize<StoredProfiles>(
                File.ReadAllText(filePath),
                JsonOptions) ?? throw new JsonException();
            if (stored.SchemaVersion != SchemaVersion)
            {
                throw new JsonException();
            }

            return [.. SwitchControlProfiles.BuiltIns, .. NormalizeCustom(stored.Profiles)];
        }
        catch
        {
            warn("Switchify PC Switch Control profiles could not be loaded. Built-in profiles will be used.");
            return SwitchControlProfiles.BuiltIns;
        }
    }

    public IReadOnlyList<SwitchControlProfile> Save(IReadOnlyList<SwitchControlProfile> customProfiles)
    {
        IReadOnlyList<SwitchControlProfile> normalized = NormalizeCustom(customProfiles);
        string json = JsonSerializer.Serialize(new StoredProfiles(SchemaVersion, normalized), JsonOptions) + "\n";
        JsonFileStore.WriteJsonFileAtomicSync(filePath, json);
        return [.. SwitchControlProfiles.BuiltIns, .. normalized];
    }

    public static IReadOnlyList<SwitchControlProfile> NormalizeCustom(IReadOnlyList<SwitchControlProfile> profiles)
    {
        if (profiles.Count > SwitchControlProfiles.MaximumCustomProfiles)
        {
            throw new InvalidDataException("Too many custom profiles.");
        }

        var names = new HashSet<string>(
            SwitchControlProfiles.BuiltIns.Select(profile => profile.Name),
            StringComparer.OrdinalIgnoreCase);
        var ids = new HashSet<string>(SwitchControlProfiles.BuiltIns.Select(profile => profile.Id), StringComparer.Ordinal);
        var normalized = new List<SwitchControlProfile>(profiles.Count);
        foreach (SwitchControlProfile profile in profiles)
        {
            string name = profile.Name.Trim();
            if (name.Length is < 1 or > 50 || !names.Add(name))
            {
                throw new InvalidDataException("Profile names must be unique and contain 1 to 50 characters.");
            }

            if (!Guid.TryParse(profile.Id, out _) || !ids.Add(profile.Id) || profile.Version < 1)
            {
                throw new InvalidDataException("Custom profile metadata is invalid.");
            }

            if (profile.Kind != SwitchControlProviderKind.Mapped || profile.Bindings.Count != 8)
            {
                throw new InvalidDataException("Custom profiles must contain eight mapped bindings.");
            }

            SwitchControlBinding[] bindings = profile.Bindings
                .OrderBy(binding => binding.SwitchId)
                .Select(NormalizeBinding)
                .ToArray();
            if (!bindings.Select(binding => binding.SwitchId).SequenceEqual(Enumerable.Range(1, 8)))
            {
                throw new InvalidDataException("Switch IDs must be 1 through 8.");
            }

            normalized.Add(profile with { Name = name, Bindings = bindings, IsBuiltIn = false });
        }

        return normalized;
    }

    private static SwitchControlBinding NormalizeBinding(SwitchControlBinding binding)
    {
        HashSet<string> keys = AllowedKeys;
        return binding.Type switch
        {
            SwitchBindingType.None => new(binding.SwitchId, SwitchBindingType.None),
            SwitchBindingType.Key when binding.Value is not null && keys.Contains(binding.Value) =>
                new(binding.SwitchId, binding.Type, binding.Value),
            SwitchBindingType.MouseButton when AllowedMouseButtons.Contains(binding.Value ?? "") =>
                new(binding.SwitchId, binding.Type, binding.Value),
            SwitchBindingType.Shortcut => NormalizeShortcut(binding, keys),
            SwitchBindingType.MouseClick when AllowedMouseButtons.Contains(binding.Value ?? "") && binding.ClickCount is 1 or 2 =>
                new(binding.SwitchId, binding.Type, binding.Value, ClickCount: binding.ClickCount),
            SwitchBindingType.Scroll when AllowedScrollDirections.Contains(binding.Value ?? "") =>
                new(binding.SwitchId, binding.Type, binding.Value),
            SwitchBindingType.Media when AllowedMediaActions.Contains(binding.Value ?? "") =>
                new(binding.SwitchId, binding.Type, binding.Value),
            _ => throw new InvalidDataException("A profile binding is invalid.")
        };
    }

    private static SwitchControlBinding NormalizeShortcut(SwitchControlBinding binding, HashSet<string> allowedKeys)
    {
        string[] keys = (binding.Keys ?? []).ToArray();
        if (keys.Length is < 1 or > 4 ||
            keys.Distinct(StringComparer.OrdinalIgnoreCase).Count() != keys.Length ||
            keys.Any(key => !allowedKeys.Contains(key)) ||
            keys.All(ModifierKeys.Contains))
        {
            throw new InvalidDataException("A shortcut binding is invalid.");
        }

        return new(binding.SwitchId, SwitchBindingType.Shortcut, Keys: keys);
    }

    private static readonly HashSet<string> ModifierKeys = new(
        ["Ctrl", "Alt", "Shift", "Meta"],
        StringComparer.OrdinalIgnoreCase);

    private static readonly HashSet<string> AllowedKeys = new(
        [
            .. Enumerable.Range('A', 26).Select(value => ((char)value).ToString()),
            .. Enumerable.Range(0, 10).Select(value => value.ToString()),
            .. Enumerable.Range(1, 12).Select(value => $"F{value}"),
            "Space", "Enter", "Escape", "Tab", "Backspace", "Delete",
            "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown",
            "Ctrl", "Alt", "Shift", "Meta"
        ],
        StringComparer.OrdinalIgnoreCase);

    private static readonly HashSet<string> AllowedMouseButtons = new(["left", "right", "middle"], StringComparer.Ordinal);
    private static readonly HashSet<string> AllowedScrollDirections = new(["up", "down", "left", "right"], StringComparer.Ordinal);
    private static readonly HashSet<string> AllowedMediaActions = new(
        ["playPause", "nextTrack", "previousTrack", "volumeUp", "volumeDown", "mute"],
        StringComparer.Ordinal);

    private sealed record StoredProfiles(int SchemaVersion, IReadOnlyList<SwitchControlProfile> Profiles);

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        PropertyNameCaseInsensitive = true,
        WriteIndented = true
    };
}
