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
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern bool PostThreadMessage(uint thread, uint message, IntPtr w, IntPtr l);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool GetTokenInformation(IntPtr token, int info, out int value, int size, out int returned);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    delegate IntPtr Hook(int code, IntPtr w, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] struct Key { public uint vk, scan, flags, time; public UIntPtr extra; }
    static readonly string Output=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"SwitchifyKeyboardQualification.log");
    static readonly Hook Callback=OnKey;
    static IntPtr HookHandle;
    static IntPtr EditorWindow;
    static volatile bool Recording;
    static readonly ConcurrentQueue<string> Events=new ConcurrentQueue<string>();
    static int EventCount, Overflow;
    static uint HookThread;
    static bool FlushEvents() {
        string value;
        while(Events.TryDequeue(out value)) { Interlocked.Decrement(ref EventCount);Log(value); }
        if(Interlocked.Exchange(ref Overflow,0)!=0) { Recording=false;Log("BLOCKED: Event evidence overflowed or could not be recorded.");return false; }
        return true;
    }
    static AutomationElement OwnedKeyboard(DateTime opened,ref int ownedPid) {
        int expectedPid=ownedPid;
        var matches=Process.GetProcessesByName("osk").Where(p=>expectedPid==0 ? p.StartTime.ToUniversalTime()>=opened : p.Id==expectedPid).ToArray();
        if(matches.Length!=1 || matches[0].MainWindowHandle==IntPtr.Zero) return null;
        ownedPid=matches[0].Id;
        return AutomationElement.FromHandle(matches[0].MainWindowHandle);
    }
    static void Log(string value) { File.AppendAllText(Output, DateTime.UtcNow.ToString("o")+" "+value+Environment.NewLine); }
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
    [STAThread] static void Main() {
        File.WriteAllText(Output,"");
        IntPtr token;
        if(!OpenProcessToken(Process.GetCurrentProcess().Handle,8,out token)) throw new InvalidOperationException("Cannot inspect token");
        int access,returned;
        bool valid=GetTokenInformation(token,26,out access,4,out returned);
        CloseHandle(token);
        Log("uiAccess="+(valid?access:-1));
        if(!valid || access!=1) { Log("BLOCKED: UIAccess is not active."); return; }
        Application.EnableVisualStyles();
        var form=new Form { Text="Switchify disposable keyboard qualification", Width=650,Height=320,Left=80,Top=80,StartPosition=FormStartPosition.Manual };
        var editor=new TextBox { Multiline=true,Dock=DockStyle.Fill,Font=new System.Drawing.Font("Segoe UI",20) };
        form.Controls.Add(editor);
        EditorWindow=form.Handle;
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
        var timer=new System.Windows.Forms.Timer { Interval=1000 };
        int step=0,attempts=0;
        bool initiallyOpen=Process.GetProcessesByName("osk").Length>0;
        string[] ids={"1e","1e","39","1c","e"};
        AutomationElement root=null;
        DateTime opened=DateTime.MaxValue;int ownedPid=0;
        form.Shown+=(s,e)=>{
            editor.Focus();
            Log("initiallyOpen="+initiallyOpen);
            Log("beforeOpen editorFocus="+editor.Focused+" foreground="+(GetForegroundWindow()==form.Handle));
            try {
                if(!initiallyOpen) { opened=DateTime.UtcNow;Process.Start(new ProcessStartInfo(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows),"System32","osk.exe")) { UseShellExecute=true }); }
                timer.Start();
            } catch(Exception error) { Log("BLOCKED: Keyboard opening failed: "+error.Message); }
        };
        timer.Tick+=(s,e)=>{
            try {
                if(!FlushEvents()) throw new Exception("Event evidence is incomplete; sequence cancelled");
                if(root==null) {
                    if(initiallyOpen) {
                        var process=Process.GetProcessesByName("osk").FirstOrDefault();
                        if(process!=null && process.MainWindowHandle!=IntPtr.Zero) root=AutomationElement.FromHandle(process.MainWindowHandle);
                    } else root=OwnedKeyboard(opened,ref ownedPid);
                    if(root==null) { if(++attempts>15) throw new Exception("Keyboard did not open");return; }
                    var all=root.FindAll(TreeScope.Descendants,Condition.TrueCondition);
                    var visible=all.Cast<AutomationElement>().Where(x=>x.Current.ControlType==ControlType.Button && !x.Current.IsOffscreen && x.Current.IsEnabled && !x.Current.BoundingRectangle.IsEmpty).ToArray();
                    Log("elements="+all.Count+" visibleEnabledButtons="+visible.Length);
                    foreach(var key in visible) { object pattern;Log("button id="+key.Current.AutomationId+" name="+key.Current.Name+" invoke="+key.TryGetCurrentPattern(InvokePattern.Pattern,out pattern)); }
                    if(visible.Length<20) throw new Exception("UIAccess key discovery failed");
                    Log("afterOpen editorFocus="+editor.Focused+" foreground="+(GetForegroundWindow()==form.Handle));
                    if(!editor.Focused || GetForegroundWindow()!=form.Handle) throw new Exception("Opening OSK did not preserve disposable editor focus");
                    Recording=true;
                    return;
                }
                if(step<ids.Length) {
                    if(!editor.Focused || GetForegroundWindow()!=form.Handle) throw new Exception("Disposable editor lost focus; activation cancelled");
                    string id=ids[step];
                    var found=root.FindAll(TreeScope.Descendants,new PropertyCondition(AutomationElement.AutomationIdProperty,id)).Cast<AutomationElement>().Where(x=>!x.Current.IsOffscreen && x.Current.IsEnabled && !x.Current.BoundingRectangle.IsEmpty).ToArray();
                    if(found.Length!=1) throw new Exception("Ambiguous or missing key "+id+" matches="+found.Length);
                    object pattern;
                    if(!found[0].TryGetCurrentPattern(InvokePattern.Pattern,out pattern)) throw new Exception("No InvokePattern for "+id);
                    Log("before id="+id+" editorFocus="+editor.Focused+" foreground="+(GetForegroundWindow()==form.Handle)+" length="+editor.TextLength);
                    ((InvokePattern)pattern).Invoke();
                    step++;return;
                }
                Log("after editorFocus="+editor.Focused+" foreground="+(GetForegroundWindow()==form.Handle)+" length="+editor.TextLength);
                Recording=false;timer.Stop();
                Log("COMPLETE: Inspect disposable text and close this window.");
            } catch(Exception error) {
                Recording=false;timer.Stop();Log("BLOCKED: "+error.GetType().Name+": "+error.Message);
            }
        };
        form.Deactivate+=(s,e)=>{if(Recording){Recording=false;timer.Stop();Log("BLOCKED: Disposable editor lost focus; sequence cancelled.");}};
        form.FormClosed+=(s,e)=>{
            Recording=false;timer.Stop();PostThreadMessage(HookThread,0x0012,IntPtr.Zero,IntPtr.Zero);
            if(!hookThread.Join(1500)) Log("BLOCKED: Hook shutdown timed out.");
            FlushEvents();
            if(!initiallyOpen && opened!=DateTime.MaxValue) {
                try {
                    AutomationElement owned=null;
                    for(int retry=0;retry<20 && owned==null;retry++) { owned=OwnedKeyboard(opened,ref ownedPid);if(owned==null) Thread.Sleep(100); }
                    object pattern;
                    if(owned!=null && owned.TryGetCurrentPattern(WindowPattern.Pattern,out pattern)) { ((WindowPattern)pattern).Close();Log("keyboardClose=invoked; verify disappearance"); }
                    else Log("keyboardClose=unavailable; restore manually if still visible");
                } catch(Exception error) { Log("keyboardClose="+error.GetType().Name+"; restore manually if still visible"); }
            }
        };
        Application.Run(form);
    }
}
