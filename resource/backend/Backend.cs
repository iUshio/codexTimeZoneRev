using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.Dynamic;
using System.IO;
using System.Linq;
using System.Net;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;
using System.Xml.Linq;

public sealed class Zone
{
    public string label, id, windowsId;
    public Zone(string label,string id,string windowsId){this.label=label;this.id=id;this.windowsId=windowsId;}
}

public sealed class Settings
{
    public string mode="zone";
    public string zoneId="Asia/Shanghai";
    public int offset=8;
    public string executable="";
    public bool dreamSkinCompatible=false;
}

public sealed class Request
{
    public string command="";
    public string dataDirectory="";
    public string launcherPath="";
    public Settings settings;
    public string path="";
}

public sealed class Reply
{
    public bool success;
    public object data;
    public string message="";
}

public static class Core
{
    public static readonly Zone[] Zones={
        new Zone("中国标准时间（上海 / 北京）","Asia/Shanghai","China Standard Time"),new Zone("太平洋时间（洛杉矶）","America/Los_Angeles","Pacific Standard Time"),
        new Zone("山地时间（丹佛）","America/Denver","Mountain Standard Time"),new Zone("美国中部时间（芝加哥）","America/Chicago","Central Standard Time"),
        new Zone("美国东部时间（纽约）","America/New_York","Eastern Standard Time"),new Zone("亚利桑那（菲尼克斯）","America/Phoenix","US Mountain Standard Time"),
        new Zone("阿拉斯加","America/Anchorage","Alaskan Standard Time"),new Zone("夏威夷","Pacific/Honolulu","Hawaiian Standard Time"),
        new Zone("英国（伦敦）","Europe/London","GMT Standard Time"),new Zone("法国（巴黎）","Europe/Paris","Romance Standard Time"),
        new Zone("德国（柏林）","Europe/Berlin","W. Europe Standard Time"),new Zone("俄罗斯（莫斯科）","Europe/Moscow","Russian Standard Time"),
        new Zone("日本（东京）","Asia/Tokyo","Tokyo Standard Time"),new Zone("韩国（首尔）","Asia/Seoul","Korea Standard Time"),
        new Zone("中国香港","Asia/Hong_Kong","China Standard Time"),new Zone("中国台北","Asia/Taipei","Taipei Standard Time"),
        new Zone("新加坡","Asia/Singapore","Singapore Standard Time"),new Zone("印度（加尔各答）","Asia/Kolkata","India Standard Time"),
        new Zone("尼泊尔（加德满都）","Asia/Kathmandu","Nepal Standard Time"),new Zone("阿联酋（迪拜）","Asia/Dubai","Arabian Standard Time"),
        new Zone("泰国（曼谷）","Asia/Bangkok","SE Asia Standard Time"),new Zone("澳大利亚（悉尼）","Australia/Sydney","AUS Eastern Standard Time"),
        new Zone("澳大利亚（阿德莱德）","Australia/Adelaide","Cen. Australia Standard Time"),new Zone("澳大利亚（珀斯）","Australia/Perth","W. Australia Standard Time"),
        new Zone("新西兰（奥克兰）","Pacific/Auckland","New Zealand Standard Time"),new Zone("巴西（圣保罗）","America/Sao_Paulo","E. South America Standard Time"),
        new Zone("协调世界时","Etc/UTC","UTC")};

    public static string Tz(Settings s)
    {
        if(s.mode=="offset"){if(s.offset < -12||s.offset > 14)throw new ArgumentException("UTC 偏移必须介于 -12 和 +14 小时。");return s.offset==0?"Etc/UTC":"Etc/GMT"+(s.offset>0?"-":"+")+Math.Abs(s.offset);}
        if(s.mode!="zone"||!Zones.Any(z=>z.id==s.zoneId))throw new ArgumentException("请选择列表中的有效时区。");return s.zoneId;
    }

    static string SettingsPath(string dataDirectory){return Path.Combine(Path.GetFullPath(dataDirectory),"settings.json");}
    public static Settings Load(string dataDirectory)
    {
        string path=SettingsPath(dataDirectory);if(!File.Exists(path)){
            string legacy=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"ChatGPTTimeZoneLauncher","settings.json");
            try{if(File.Exists(legacy)){var old=new JavaScriptSerializer().Deserialize<Dictionary<string,object>>(File.ReadAllText(legacy,Encoding.UTF8));var migrated=new Settings();if(old!=null){if(old.ContainsKey("Mode"))migrated.mode=Convert.ToString(old["Mode"]);if(old.ContainsKey("ZoneId"))migrated.zoneId=Convert.ToString(old["ZoneId"]);if(old.ContainsKey("Offset"))migrated.offset=Convert.ToInt32(old["Offset"]);if(old.ContainsKey("Executable"))migrated.executable=Convert.ToString(old["Executable"]);if(old.ContainsKey("DreamSkinCompatible"))migrated.dreamSkinCompatible=Convert.ToBoolean(old["DreamSkinCompatible"]);}Save(dataDirectory,migrated);return migrated;}}catch(UnauthorizedAccessException){}catch(IOException){}catch(InvalidOperationException){}
            return new Settings();
        }
        var settings=new JavaScriptSerializer().Deserialize<Settings>(File.ReadAllText(path,Encoding.UTF8));if(settings==null)throw new InvalidDataException("设置文件内容为空。");Tz(settings);return settings;
    }
    public static void Save(string dataDirectory,Settings settings)
    {
        Tz(settings);string path=SettingsPath(dataDirectory);Directory.CreateDirectory(Path.GetDirectoryName(path));string temp=path+"."+Guid.NewGuid().ToString("N")+".tmp";
        try{File.WriteAllText(temp,new JavaScriptSerializer().Serialize(settings),new UTF8Encoding(false));if(File.Exists(path))File.Replace(temp,path,null);else File.Move(temp,path);}finally{if(File.Exists(temp))File.Delete(temp);}
    }
    static string ManifestEntry(string root)
    {
        var doc=XDocument.Load(Path.Combine(root,"AppxManifest.xml"));var app=doc.Descendants().FirstOrDefault(x=>x.Name.LocalName=="Application"&&(string)x.Attribute("Id")=="App");if(app==null)return "";string relative=(string)app.Attribute("Executable");if(String.IsNullOrWhiteSpace(relative))return "";string path=Path.GetFullPath(Path.Combine(root,relative.Replace('/','\\')));return path.StartsWith(Path.GetFullPath(root).TrimEnd('\\')+"\\",StringComparison.OrdinalIgnoreCase)&&File.Exists(path)?path:"";
    }
    public static string Discover()
    {
        var psi=new ProcessStartInfo(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System),@"WindowsPowerShell\v1.0\powershell.exe"),"-NoProfile -NonInteractive -Command \"[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-AppxPackage -Name OpenAI.Codex | Sort-Object Version -Descending | ForEach-Object { $_.InstallLocation }\""){UseShellExecute=false,CreateNoWindow=true,RedirectStandardOutput=true,RedirectStandardError=true,StandardOutputEncoding=Encoding.UTF8};
        using(var process=Process.Start(psi)){string output=process.StandardOutput.ReadToEnd();if(!process.WaitForExit(12000)){process.Kill();throw new IOException("自动查找超时，请手动选择客户端。");}foreach(string line in output.Split(new[]{'\r','\n'},StringSplitOptions.RemoveEmptyEntries))try{string found=ManifestEntry(line.Trim());if(found!="")return found;}catch(IOException){}catch(UnauthorizedAccessException){}}
        string local=Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);foreach(string relative in new[]{@"Programs\Codex\Codex.exe",@"Codex\Codex.exe",@"Programs\OpenAI\Codex\Codex.exe"}){string path=Path.Combine(local,relative);if(File.Exists(path))return path;}return "";
    }
    public static string ValidateExecutable(string path)
    {
        if(String.IsNullOrWhiteSpace(path))throw new IOException("未找到客户端，请手动选择 Codex 桌面程序。");path=Path.GetFullPath(path.Trim().Trim('"'));if(!File.Exists(path))throw new FileNotFoundException("客户端路径不存在。",path);string name=Path.GetFileName(path).ToLowerInvariant();if((name!="codex.exe"&&name!="chatgpt.exe")||!File.Exists(Path.Combine(Path.GetDirectoryName(path),"icudtl.dat")))throw new IOException("请选择 Codex 桌面安装目录中的程序，不要选择命令行 codex.exe。");return path;
    }
    public static bool IsRunning(string executable)
    {
        string folder=Path.GetDirectoryName(Path.GetFullPath(executable));foreach(string name in new[]{"Codex","ChatGPT"})foreach(var process in Process.GetProcessesByName(name))using(process)try{if(process.SessionId==Process.GetCurrentProcess().SessionId&&String.Equals(Path.GetDirectoryName(process.MainModule.FileName),folder,StringComparison.OrdinalIgnoreCase))return true;}catch(InvalidOperationException){}catch(System.ComponentModel.Win32Exception){return true;}return false;
    }
    public static ProcessStartInfo StartInfo(string executable,Settings settings)
    {
        var psi=new ProcessStartInfo(executable){UseShellExecute=false,WorkingDirectory=Path.GetDirectoryName(executable)};psi.EnvironmentVariables["TZ"]=Tz(settings);psi.EnvironmentVariables.Remove("ELECTRON_RUN_AS_NODE");if(settings.dreamSkinCompatible)psi.Arguments=DreamSkin.Arguments(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"CodexDreamSkin"));return psi;
    }
}

public static class DreamSkin
{
    static Dictionary<string,object> ReadState(string root){string state=Path.Combine(root,"state.json");return File.Exists(state)?new JavaScriptSerializer().Deserialize<Dictionary<string,object>>(File.ReadAllText(state)):null;}
    public static int Port(string root){int port=9335;var data=ReadState(root);if(data!=null&&data.ContainsKey("port"))port=Convert.ToInt32(data["port"]);if(port<1024||port>65535)throw new InvalidDataException("Dream Skin 端口无效。");return port;}
    public static bool IsTray(string path){if(!String.Equals(Path.GetFileName(path),"tray-dream-skin.ps1",StringComparison.OrdinalIgnoreCase)||!File.Exists(path))return false;string dir=Path.GetDirectoryName(path);foreach(string dependency in new[]{"common-windows.ps1","theme-windows.ps1","localization-windows.ps1","start-dream-skin.ps1"})if(!File.Exists(Path.Combine(dir,dependency)))return false;return true;}
    public static string FindTray(string root){foreach(string path in new[]{Path.Combine(root,@"engine\scripts\tray-dream-skin.ps1"),Path.Combine(root,@"scripts\tray-dream-skin.ps1")})if(IsTray(path))return Path.GetFullPath(path);try{var data=ReadState(root);if(data!=null&&data.ContainsKey("injectorPath")){string injector=Convert.ToString(data["injectorPath"]);if(!String.IsNullOrWhiteSpace(injector)&&Path.IsPathRooted(injector)){string path=Path.Combine(Path.GetDirectoryName(injector),"tray-dream-skin.ps1");if(IsTray(path))return Path.GetFullPath(path);}}}catch(ArgumentException){}catch(InvalidOperationException){}catch(IOException){}return "";}
    public static string FindStart(string root){string tray=FindTray(root);if(tray=="")return "";string start=Path.Combine(Path.GetDirectoryName(tray),"start-dream-skin.ps1");return File.Exists(start)?Path.GetFullPath(start):"";}
    static ProcessStartInfo ScriptInfo(string script,string prefix){string full=Path.GetFullPath(script);if(full.IndexOfAny(new[]{'"','\r','\n'})>=0)throw new ArgumentException("Dream Skin 路径无效。");return new ProcessStartInfo(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System),@"WindowsPowerShell\v1.0\powershell.exe")){UseShellExecute=false,CreateNoWindow=true,WindowStyle=ProcessWindowStyle.Hidden,WorkingDirectory=Path.GetDirectoryName(Path.GetDirectoryName(full)),Arguments="-NoProfile "+prefix+" -WindowStyle Hidden -ExecutionPolicy RemoteSigned -File \""+full+"\""};}
    public static ProcessStartInfo TrayStartInfo(string script){if(!IsTray(script))throw new FileNotFoundException("Dream Skin 安装目录不完整。");return ScriptInfo(script,"-STA");}
    public static ProcessStartInfo ApplyStartInfo(string script){if(!String.Equals(Path.GetFileName(script),"start-dream-skin.ps1",StringComparison.OrdinalIgnoreCase)||!File.Exists(script))throw new FileNotFoundException("Dream Skin 官方启动脚本不存在，请重新安装该工具。");if(!IsTray(Path.Combine(Path.GetDirectoryName(Path.GetFullPath(script)),"tray-dream-skin.ps1")))throw new FileNotFoundException("Dream Skin 安装目录不完整，请重新安装该工具。");var info=ScriptInfo(script,"");info.Arguments+=" -OperationLockTimeoutMilliseconds 300000";return info;}
    public static bool WaitForEndpoint(int port,int timeout){if(port<1024||port>65535)throw new ArgumentOutOfRangeException("port");var deadline=DateTime.UtcNow.AddMilliseconds(timeout);while(DateTime.UtcNow<deadline){try{var request=(HttpWebRequest)WebRequest.Create("http://127.0.0.1:"+port+"/json/version");request.Proxy=null;request.Timeout=750;request.ReadWriteTimeout=750;request.AllowAutoRedirect=false;using(var response=(HttpWebResponse)request.GetResponse())using(var reader=new StreamReader(response.GetResponseStream())){var data=new JavaScriptSerializer().Deserialize<Dictionary<string,object>>(reader.ReadToEnd());string socket=data!=null&&data.ContainsKey("webSocketDebuggerUrl")?Convert.ToString(data["webSocketDebuggerUrl"]):"";Uri uri;if(Uri.TryCreate(socket,UriKind.Absolute,out uri)&&uri.Scheme=="ws"&&uri.Port==port&&(uri.Host=="127.0.0.1"||uri.Host=="localhost"||uri.Host=="::1"))return true;}}catch(WebException){}catch(IOException){}catch(InvalidOperationException){}catch(ArgumentException){}Thread.Sleep(250);}return false;}
    public static string Arguments(string root){int port=Port(root);string profile=Path.Combine(root,"cdp-profile");var data=ReadState(root);if(data!=null&&data.ContainsKey("profilePath")&&!String.IsNullOrWhiteSpace(Convert.ToString(data["profilePath"])))profile=Convert.ToString(data["profilePath"]);profile=Path.GetFullPath(profile).TrimEnd('\\');return "--remote-debugging-address=127.0.0.1 --remote-debugging-port="+port+" \"--user-data-dir="+profile+"\"";}
}

public static class Backend
{
    static readonly JavaScriptSerializer Json=new JavaScriptSerializer();
    static object Execute(Request request)
    {
        if(String.IsNullOrWhiteSpace(request.dataDirectory))throw new ArgumentException("未提供数据目录。");
        switch(request.command){
            case "bootstrap":return new{settings=Core.Load(request.dataDirectory),zones=Core.Zones,localZone=TimeZoneInfo.Local.Id,detected=SafeDiscover()};
            case "discover":return new{path=Core.Discover()};
            case "validate":return new{path=Core.ValidateExecutable(request.path)};
            case "save":Core.Save(request.dataDirectory,request.settings);return new{path=Path.Combine(request.dataDirectory,"settings.json")};
            case "launch":return Launch(request);
            case "launch_dream_skin":return LaunchDreamSkin();
            case "create_shortcut":CreateShortcut(request.launcherPath);return new{created=true};
            default:throw new ArgumentException("未知命令："+request.command);
        }
    }
    static string SafeDiscover(){try{return Core.Discover();}catch{return "";}}
    static object Launch(Request request)
    {
        Settings settings=request.settings??Core.Load(request.dataDirectory);Core.Save(request.dataDirectory,settings);string path=String.IsNullOrWhiteSpace(settings.executable)?Core.Discover():settings.executable;path=Core.ValidateExecutable(path);if(Core.IsRunning(path))throw new InvalidOperationException("客户端正在运行，请保存工作并完全退出 Codex 后重试。");
        string skinRoot=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"CodexDreamSkin"),skinStart="";int skinPort=0;if(settings.dreamSkinCompatible){skinStart=DreamSkin.FindStart(skinRoot);if(skinStart=="")throw new FileNotFoundException("未找到完整的 Dream Skin 安装。");skinPort=DreamSkin.Port(skinRoot);}
        using(var process=Process.Start(Core.StartInfo(path,settings))){if(process==null)throw new IOException("未能创建客户端进程。");}
        if(settings.dreamSkinCompatible){if(!DreamSkin.WaitForEndpoint(skinPort,30000))throw new IOException("Codex 已启动，但 Dream Skin 调试端点未在 30 秒内就绪。");using(var apply=Process.Start(DreamSkin.ApplyStartInfo(skinStart))){if(apply==null)throw new IOException("无法启动 Dream Skin 官方应用流程。");if(!apply.WaitForExit(130000))return new{launched=true,message="Codex 已启动，Dream Skin 仍在后台验证皮肤。"};if(apply.ExitCode!=0)throw new IOException("Dream Skin 应用失败，退出码 "+apply.ExitCode+"。");}}
        return new{launched=true,message=settings.dreamSkinCompatible?"Codex 已按所选时区启动，Dream Skin 已应用皮肤。":"Codex 已按所选时区启动。"};
    }
    static object LaunchDreamSkin(){string root=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"CodexDreamSkin"),tray=DreamSkin.FindTray(root);if(tray=="")throw new FileNotFoundException("未找到完整的 Dream Skin 安装。");using(var process=Process.Start(DreamSkin.TrayStartInfo(tray))){if(process==null)throw new IOException("无法启动 Dream Skin。");}return new{launched=true,path=tray};}
    static void CreateShortcut(string launcherPath){launcherPath=Path.GetFullPath(launcherPath);Type type=Type.GetTypeFromProgID("WScript.Shell");dynamic shell=Activator.CreateInstance(type);string path=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory),"Codex 时区启动器.lnk");dynamic link=shell.CreateShortcut(path);link.TargetPath=launcherPath;link.IconLocation=launcherPath+",0";link.WorkingDirectory=Path.GetDirectoryName(launcherPath);link.Description="选择时区并启动 Codex";link.Save();}
    [STAThread]public static int Main(string[] args)
    {
        Console.OutputEncoding=new UTF8Encoding(false);try{if(args.Length!=1)throw new ArgumentException("需要一个 Base64 JSON 请求参数。");string text=Encoding.UTF8.GetString(Convert.FromBase64String(args[0]));var request=Json.Deserialize<Request>(text);Console.Write(Json.Serialize(new Reply{success=true,data=Execute(request)}));return 0;}catch(Exception error){Console.Write(Json.Serialize(new Reply{success=false,message=error.Message}));return 1;}
    }
}
