using System;

internal static class KeyboardQualificationTests
{
    static void Require(bool condition, string message) {
        if(!condition) throw new Exception(message);
    }
    static void Main() {
        int waits=0,lookups=0;
        var window=WindowsProbe.WaitForWindow(()=>false,()=>++lookups==4?new IntPtr(42):IntPtr.Zero,()=>waits++);
        Require(window==new IntPtr(42) && waits==3,"A delayed OSK window must still be found during early-close cleanup.");

        waits=0;lookups=0;
        window=WindowsProbe.WaitForWindow(()=>waits==2,()=>{lookups++;return IntPtr.Zero;},()=>waits++);
        Require(window==IntPtr.Zero && waits==2 && lookups==2,"Process exit must stop discovery without another lookup.");

        waits=0;
        window=WindowsProbe.WaitForWindow(()=>false,()=>IntPtr.Zero,()=>waits++);
        Require(window==IntPtr.Zero && waits==30,"A keyboard that never creates a window must time out.");

        waits=0;
        window=WindowsProbe.WaitForWindow(()=>false,()=>new IntPtr(43),()=>waits++);
        Require(window==new IntPtr(43) && waits==0,"An existing owned window should not wait.");
        Console.WriteLine("4 fake keyboard lifecycle tests passed. No native input or windows used.");
    }
}
