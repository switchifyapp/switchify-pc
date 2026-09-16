using System;
using System.Diagnostics;
using System.Collections.Concurrent;
using System.Threading;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows.Automation;
using System.Windows.Forms;

internal static class WindowsProbe
{
    [DllImport("user32.dll")] static extern IntPtr SetWindowsHookEx(int id, Hook callback, IntPtr module, uint thread);
    [DllImport("user32.dll")] static extern bool UnhookWindowsHookEx(IntPtr hook);
    [DllImport("user32.dll")] static extern IntPtr CallNextHookEx(IntPtr hook, int code, IntPtr w, IntPtr l);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode)] static extern IntPtr GetModuleHandle(string name);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll", SetLastError=true)] static extern bool PostMessage(IntPtr window, uint message, IntPtr w, IntPtr l);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern bool PostThreadMessage(uint thread, uint message, IntPtr w, IntPtr l);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool GetTokenInformation(IntPtr token, int info, out int value, int size, out int returned);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    [DllImport("oleacc.dll")] static extern int AccessibleObjectFromPoint(System.Drawing.Point point, [MarshalAs(UnmanagedType.Interface)] out Accessibility.IAccessible accessible, [MarshalAs(UnmanagedType.Struct)] out object child);
    [DllImport("oleacc.dll")] static extern int WindowFromAccessibleObject(Accessibility.IAccessible accessible, out IntPtr window);
    delegate IntPtr Hook(int code, IntPtr w, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] struct Key { public uint vk, scan, flags, time; public UIntPtr extra; }
    static readonly string Output=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"SwitchifyKeyboardQualification.log");
    static readonly object LogLock=new object();
    static readonly Hook Callback=OnKey;
    static readonly ManualResetEventSlim Cancelled=new ManualResetEventSlim();
    static readonly ConcurrentQueue<string> Events=new ConcurrentQueue<string>();
    static IntPtr HookHandle, EditorWindow;
    static volatile bool Recording;
    static int EventCount, Overflow;
    static uint HookThread;
    static Form Form;
    static TextBox Editor;
    static Process Owned;
    static IntPtr OwnedWindow;
    static DateTime Opened=DateTime.MaxValue;
    static bool InitiallyOpen;
    static bool LegacyActivation;

    static void ActivateKey(AutomationElement key, InvokePattern invoke) {
        if(!LegacyActivation) { invoke.Invoke();return; }
        var bounds=key.Current.BoundingRectangle;
        var point=new System.Drawing.Point((int)(bounds.X+bounds.Width/2),(int)(bounds.Y+bounds.Height/2));
        Accessibility.IAccessible accessible;
        object child;
        Marshal.ThrowExceptionForHR(AccessibleObjectFromPoint(point,out accessible,out child));
        try {
            IntPtr window;
            Marshal.ThrowExceptionForHR(WindowFromAccessibleObject(accessible,out window));
            uint process;GetWindowThreadProcessId(window,out process);
            int x,y,width,height;
            accessible.accLocation(out x,out y,out width,out height,child);
            if(process!=(uint)key.Current.ProcessId || accessible.get_accName(child)!=key.Current.Name || Convert.ToInt32(accessible.get_accRole(child))!=0x2b ||
               point.X<x || point.X>=x+width || point.Y<y || point.Y>=y+height)
                throw new InvalidOperationException("MSAA key identity did not match the discovered button");
            CheckCancelled();
            Snapshot("beforeNativeDefaultAction");
            accessible.accDoDefaultAction(child);
        } finally { if(accessible!=null) Marshal.ReleaseComObject(accessible); }
    }

    static void Log(string value) { lock(LogLock) File.AppendAllText(Output,DateTime.UtcNow.ToString("o")+" "+value+Environment.NewLine); }
    static bool FlushEvents() {
        string value;
        while(Events.TryDequeue(out value)) { Interlocked.Decrement(ref EventCount);Log(value); }
        if(Interlocked.Exchange(ref Overflow,0)!=0) { Cancelled.Set();Log("BLOCKED: Event evidence overflowed or could not be recorded.");return false; }
        return true;
    }
    static IntPtr OnKey(int code,IntPtr w,IntPtr l) {
        try {
            if(code>=0 && Recording && GetForegroundWindow()==EditorWindow) {
                if(Interlocked.Increment(ref EventCount)<=512) {
                    var key=(Key)Marshal.PtrToStructure(l,typeof(Key));
                    Events.Enqueue("key time="+key.time+" message="+w.ToInt64()+" vk="+key.vk+" flags="+key.flags+" extra="+key.extra.ToUInt64());
                } else { Interlocked.Decrement(ref EventCount);Interlocked.Exchange(ref Overflow,1);Recording=false; }
            }
        } catch { Interlocked.Exchange(ref Overflow,1);Recording=false; }
        return CallNextHookEx(HookHandle,code,w,l);
    }
    static void CheckCancelled() { if(Cancelled.IsSet) throw new OperationCanceledException(); }
    static void Wait(int milliseconds) { if(Cancelled.Wait(milliseconds)) throw new OperationCanceledException(); }
    static int Snapshot(string label, string expected=null) {
        CheckCancelled();
        return (int)Form.Invoke(new Func<int>(()=>{
            var foreground=GetForegroundWindow();
            uint process;GetWindowThreadProcessId(foreground,out process);
            bool focused=Editor.Focused && foreground==EditorWindow;
            Log(label+" editorFocus="+Editor.Focused+" foreground="+(foreground==EditorWindow)+" foregroundPid="+process+" length="+Editor.TextLength);
            if(!focused) throw new InvalidOperationException("Disposable editor lost focus; activation cancelled");
            if(expected!=null && !String.Equals(Editor.Text,expected,StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException("Disposable text did not match the expected test sequence");
            return Editor.TextLength;
        }));
    }
    // UIA objects remain on this windowless MTA thread. Never reclaim focus after
    // a failed invocation; that would hide a native keyboard integration failure.
    static void RunSequence() {
        try {
            Log("automationApartment="+Thread.CurrentThread.GetApartmentState());
            Snapshot("beforeOpen");
            if(!InitiallyOpen) {
                CheckCancelled();
                Opened=DateTime.UtcNow;
                Process.Start(new ProcessStartInfo(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows),"System32","osk.exe")) { UseShellExecute=true });
            }
            AutomationElement root=null;
            for(int attempt=0;attempt<15 && root==null;attempt++) {
                Wait(1000);
                var candidates=Process.GetProcessesByName("osk").Where(p=>InitiallyOpen || p.StartTime.ToUniversalTime()>=Opened).ToArray();
                if(candidates.Length>1) throw new InvalidOperationException("Multiple keyboard processes; ownership is ambiguous");
                if(candidates.Length==1 && candidates[0].MainWindowHandle!=IntPtr.Zero) {
                    var process=candidates[0];
                    if(!InitiallyOpen) { var handle=process.Handle;Owned=process;OwnedWindow=process.MainWindowHandle; }
                    root=AutomationElement.FromHandle(process.MainWindowHandle);
                    Log("keyboardPid="+process.Id);
                }
            }
            if(root==null) throw new InvalidOperationException("Keyboard did not open");
            CheckCancelled();
            var all=root.FindAll(TreeScope.Descendants,Condition.TrueCondition);
            var visible=all.Cast<AutomationElement>().Where(x=>x.Current.ControlType==ControlType.Button && !x.Current.IsOffscreen && x.Current.IsEnabled && !x.Current.BoundingRectangle.IsEmpty).ToArray();
            Log("elements="+all.Count+" visibleEnabledButtons="+visible.Length);
            foreach(var key in visible) { object pattern;Log("button id="+key.Current.AutomationId+" name="+key.Current.Name+" invoke="+key.TryGetCurrentPattern(InvokePattern.Pattern,out pattern)); }
            if(visible.Length<20) throw new InvalidOperationException("UIAccess key discovery failed");
            Snapshot("afterOpen");
            Recording=true;
            string[] ids={"1e","1e","39","1c","e"};
            string expected="";
            foreach(string id in ids) {
                Wait(1000);
                if(!FlushEvents()) throw new InvalidOperationException("Event evidence is incomplete");
                var found=root.FindAll(TreeScope.Descendants,new PropertyCondition(AutomationElement.AutomationIdProperty,id)).Cast<AutomationElement>().Where(x=>!x.Current.IsOffscreen && x.Current.IsEnabled && !x.Current.BoundingRectangle.IsEmpty).ToArray();
                if(found.Length!=1) throw new InvalidOperationException("Ambiguous or missing key "+id+" matches="+found.Length);
                object pattern;
                if(!found[0].TryGetCurrentPattern(InvokePattern.Pattern,out pattern)) throw new InvalidOperationException("No InvokePattern for "+id);
                int before=Snapshot("before id="+id);
                CheckCancelled();
                ActivateKey(found[0],(InvokePattern)pattern);
                Wait(300);
                expected=id=="e"?"aa ":expected+(id=="1c"?"\r\n":id=="39"?" ":"a");
                int after=Snapshot("after id="+id,expected);
                int delta=id=="1c"?2:id=="e"?-2:1;
                if(after-before!=delta) throw new InvalidOperationException("Unexpected text-length change for "+id+": "+(after-before));
            }
            Wait(300);
            if(!FlushEvents()) throw new InvalidOperationException("Event evidence is incomplete");
            Snapshot("afterSequence");
            Log("COMPLETE: Focus and text-length checks passed. Event identification still requires review.");
        } catch(OperationCanceledException) { Log("CANCELLED: No further key requests will be sent."); }
        catch(Exception error) { Log("BLOCKED: "+error.GetType().Name+": "+error.Message); }
        finally {
            Recording=false;
            if(!Form.IsDisposed) { try { Form.BeginInvoke(new Action(()=>Form.Close())); } catch(InvalidOperationException) {} }
        }
    }
    static void CloseOwnedKeyboard() {
        if(InitiallyOpen) { Log("keyboardClose=preserved-preexisting");return; }
        // Find a late-created OSK after cancellation, but never close an older or
        // ambiguous process. Retain its process handle to guard against PID reuse.
        if(Owned==null && Opened!=DateTime.MaxValue) {
            for(int retry=0;retry<20 && Owned==null;retry++) {
                var candidates=Process.GetProcessesByName("osk").Where(p=>p.StartTime.ToUniversalTime()>=Opened).ToArray();
                if(candidates.Length>1) { Log("BLOCKED: Keyboard cleanup ownership ambiguous.");return; }
                if(candidates.Length==1) { var handle=candidates[0].Handle;Owned=candidates[0];OwnedWindow=Owned.MainWindowHandle; }
                else Thread.Sleep(100);
            }
        }
        if(Owned==null) { Log("keyboardClose=no-owned-process");return; }
        try {
            OwnedWindow=WaitForWindow(()=>Owned.HasExited,()=>{Owned.Refresh();return Owned.MainWindowHandle;},()=>Thread.Sleep(100));
            if(Owned.HasExited) { Log("keyboardClose=verified-exited");return; }
            uint process;GetWindowThreadProcessId(OwnedWindow,out process);
            if(OwnedWindow==IntPtr.Zero || process!=(uint)Owned.Id) { Log("BLOCKED: Owned keyboard window is unavailable.");return; }
            // SC_CLOSE follows the native window close path instead of relying on
            // WindowPattern.Close returning before the keyboard disappears.
            if(!PostMessage(OwnedWindow,0x0112,new IntPtr(0xF060),IntPtr.Zero)) { Log("BLOCKED: Native keyboard close failed error="+Marshal.GetLastWin32Error());return; }
            for(int attempt=0;attempt<30;attempt++) {
                if(Owned.HasExited || !IsWindowVisible(OwnedWindow)) { Log("keyboardClose=verified-disappeared");return; }
                Thread.Sleep(100);
            }
            Log("BLOCKED: Keyboard remained visible after native close.");
        } finally { Owned.Dispose(); }
    }
    // Pure retry policy: fake callbacks exercise delayed windows and cancellation
    // cleanup in CI without creating a window or sending any native input.
    internal static IntPtr WaitForWindow(Func<bool> exited, Func<IntPtr> findWindow, Action wait) {
        for(int attempt=0;attempt<30;attempt++) {
            if(exited()) return IntPtr.Zero;
            var window=findWindow();
            if(window!=IntPtr.Zero) return window;
            wait();
        }
        return IntPtr.Zero;
    }
    [STAThread] static void Main(string[] args) {
        File.WriteAllText(Output,"");
        if(args.Any(arg=>arg!="--activation=invoke")) { Log("BLOCKED: Unknown argument.");return; }
        LegacyActivation=!args.Contains("--activation=invoke");
        Log("activation="+(LegacyActivation?"MSAA default action":"UIA InvokePattern"));
        IntPtr token;
        if(!OpenProcessToken(Process.GetCurrentProcess().Handle,8,out token)) throw new InvalidOperationException("Cannot inspect token");
        int access,returned;
        bool valid=GetTokenInformation(token,26,out access,4,out returned);
        CloseHandle(token);
        Log("uiAccess="+(valid?access:-1));
        if(!valid || access!=1) { Log("BLOCKED: UIAccess is not active.");return; }
        Application.EnableVisualStyles();
        Form=new Form { Text="Switchify disposable keyboard qualification",Width=650,Height=320,Left=80,Top=80,StartPosition=FormStartPosition.Manual };
        Editor=new TextBox { Multiline=true,Dock=DockStyle.Fill,Font=new System.Drawing.Font("Segoe UI",20) };
        Form.Controls.Add(Editor);
        EditorWindow=Form.Handle;
        var ready=new ManualResetEventSlim();
        var hookThread=new Thread(()=>{
            HookThread=GetCurrentThreadId();
            HookHandle=SetWindowsHookEx(13,Callback,GetModuleHandle(null),0);
            ready.Set();
            if(HookHandle!=IntPtr.Zero) { try { Application.Run(); } finally { UnhookWindowsHookEx(HookHandle); } }
        });
        hookThread.IsBackground=true;hookThread.SetApartmentState(ApartmentState.STA);hookThread.Start();
        if(!ready.Wait(3000)) { Log("BLOCKED: Hook startup timed out.");return; }
        Log("hook="+(HookHandle!=IntPtr.Zero));
        if(HookHandle==IntPtr.Zero) return;
        InitiallyOpen=Process.GetProcessesByName("osk").Length>0;
        Log("initiallyOpen="+InitiallyOpen);
        var worker=new Thread(RunSequence) { IsBackground=true };
        worker.SetApartmentState(ApartmentState.MTA);
        var watchdog=new System.Windows.Forms.Timer { Interval=30000 };
        watchdog.Tick+=(s,e)=>{Log("BLOCKED: Qualification timed out.");Form.Close();};
        Form.Shown+=(s,e)=>{Editor.Focus();worker.Start();watchdog.Start();};
        Form.Deactivate+=(s,e)=>{
            if(Recording) {
                uint process;GetWindowThreadProcessId(GetForegroundWindow(),out process);
                Log("BLOCKED: Disposable editor lost focus; foregroundPid="+process);
                Recording=false;Cancelled.Set();
            }
        };
        Form.FormClosing+=(s,e)=>{Recording=false;Cancelled.Set();watchdog.Stop();};
        Application.Run(Form);
        PostThreadMessage(HookThread,0x0012,IntPtr.Zero,IntPtr.Zero);
        if(!hookThread.Join(1500)) Log("BLOCKED: Hook shutdown timed out.");
        if(!worker.Join(1500)) Log("BLOCKED: UIA shutdown timed out; native qualification is incomplete.");
        FlushEvents();
        CloseOwnedKeyboard();
    }
}
