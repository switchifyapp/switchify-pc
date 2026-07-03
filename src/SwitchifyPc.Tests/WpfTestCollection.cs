using Xunit;

namespace SwitchifyPc.Tests;

[CollectionDefinition(Name, DisableParallelization = true)]
public sealed class WpfTestCollection
{
    public const string Name = "WPF tests";
}
