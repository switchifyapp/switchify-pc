using System.Threading;
using System.Windows;
using SwitchifyPc.App;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class IndependentWindowHostTests
{
    [Fact]
    public void ReusesRestoresAndKeepsWindowIndependent()
    {
        RunOnSta(() =>
        {
            int created = 0;
            Window owner = new();
            owner.Show();
            IndependentWindowHost<Window> host = new(() =>
            {
                created++;
                return new Window { Owner = owner };
            });

            Window first = host.Show();
            first.WindowState = WindowState.Minimized;
            Window second = host.Show();

            Assert.Same(first, second);
            Assert.Equal(1, created);
            Assert.Null(second.Owner);
            Assert.Equal(WindowState.Normal, second.WindowState);

            second.Close();
            Window third = host.Show();
            Assert.NotSame(first, third);
            Assert.Equal(2, created);
            third.Close();
            owner.Close();
        });
    }

    private static void RunOnSta(Action action)
    {
        Exception? error = null;
        Thread thread = new(() =>
        {
            try
            {
                action();
            }
            catch (Exception exception)
            {
                error = exception;
            }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();
        if (error is not null) throw error;
    }
}
