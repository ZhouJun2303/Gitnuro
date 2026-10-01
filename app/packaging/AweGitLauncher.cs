using System;
using System.Diagnostics;
using System.IO;

static class Program
{
    static int Main()
    {
        string dir = AppDomain.CurrentDomain.BaseDirectory.TrimEnd(
            Path.DirectorySeparatorChar,
            Path.AltDirectorySeparatorChar);
        string java = Path.Combine(dir, "runtime", "bin", "javaw.exe");
        if (!File.Exists(java))
        {
            Console.Error.WriteLine("Cannot find " + java);
            return 1;
        }

        Process.Start(new ProcessStartInfo
        {
            FileName = java,
            Arguments = "--enable-native-access=ALL-UNNAMED -cp \"app\\*\" com.zhoujun.awegit.MainKt",
            WorkingDirectory = dir,
            UseShellExecute = false,
        });
        return 0;
    }
}
