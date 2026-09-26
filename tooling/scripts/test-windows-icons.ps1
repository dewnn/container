param([string]$Executable = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'src-tauri\target\release\container-studio-dev.exe'))

$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not ('ContainerIconResources' -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Runtime.InteropServices;

public static class ContainerIconResources {
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  static extern IntPtr LoadLibraryEx(string path, IntPtr file, uint flags);
  [DllImport("kernel32.dll", SetLastError = true)]
  static extern bool FreeLibrary(IntPtr module);
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  static extern IntPtr FindResource(IntPtr module, IntPtr name, IntPtr type);
  [DllImport("kernel32.dll")] static extern uint SizeofResource(IntPtr module, IntPtr resource);
  [DllImport("kernel32.dll")] static extern IntPtr LoadResource(IntPtr module, IntPtr resource);
  [DllImport("kernel32.dll")] static extern IntPtr LockResource(IntPtr resource);
  [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
  static extern uint ExtractIconEx(string file, int index, out IntPtr large, out IntPtr small, uint count);
  [DllImport("user32.dll")] static extern bool DestroyIcon(IntPtr icon);

  static byte[] Resource(IntPtr module, int name, int type) {
    IntPtr info = FindResource(module, new IntPtr(name), new IntPtr(type));
    if (info == IntPtr.Zero) throw new Exception("Missing icon resource " + type + ":" + name);
    byte[] bytes = new byte[SizeofResource(module, info)];
    IntPtr data = LockResource(LoadResource(module, info));
    if (data == IntPtr.Zero) throw new Exception("Cannot load icon resource " + name);
    Marshal.Copy(data, bytes, 0, bytes.Length);
    return bytes;
  }

  static void Equal(byte[] a, int offsetA, byte[] b, int offsetB, int count) {
    for (int i = 0; i < count; i++)
      if (a[offsetA + i] != b[offsetB + i]) throw new Exception("Embedded icon differs at source byte " + (offsetA + i) + ", resource byte " + (offsetB + i) + ": " + a[offsetA + i] + " != " + b[offsetB + i]);
  }

  static void CheckGroup(IntPtr module, int id, string source) {
    byte[] ico = File.ReadAllBytes(source);
    byte[] group = Resource(module, id, 14);
    Equal(ico, 0, group, 0, 6);
    int count = BitConverter.ToUInt16(ico, 4);
    if (count != 7 || group.Length != 6 + count * 14) throw new Exception("Expected all seven icon sizes");
    for (int i = 0; i < count; i++) {
      int entry = 6 + i * 16, resourceEntry = 6 + i * 14;
      Equal(ico, entry, group, resourceEntry, 4);
      // Pillow leaves directory planes at zero; RC fills it from the DIB.
      // Compare with the actual bitmap header, not that optional ICO hint.
      int imageOffset = BitConverter.ToInt32(ico, entry + 12);
      Equal(ico, imageOffset + 12, group, resourceEntry + 4, 2);
      Equal(ico, entry + 6, group, resourceEntry + 6, 6);
      byte[] frame = Resource(module, BitConverter.ToUInt16(group, resourceEntry + 12), 3);
      if (frame.Length != BitConverter.ToInt32(ico, entry + 8)) throw new Exception("Icon frame size mismatch");
      Equal(ico, BitConverter.ToInt32(ico, entry + 12), frame, 0, frame.Length);
    }
  }

  public static void Verify(string exe, string appIcon, string projectIcon) {
    IntPtr module = LoadLibraryEx(exe, IntPtr.Zero, 0x22);
    if (module == IntPtr.Zero) throw new Exception("Cannot read executable resources: " + Marshal.GetLastWin32Error());
    try {
      CheckGroup(module, 32512, appIcon);
      CheckGroup(module, 32513, projectIcon);
    } finally { FreeLibrary(module); }
    foreach (int index in new int[] { 0, -32512, -32513 }) {
      IntPtr large, small;
      uint count = ExtractIconEx(exe, index, out large, out small, 1);
      try {
        if (count != 2 || large == IntPtr.Zero || small == IntPtr.Zero)
          throw new Exception("Windows Shell icon extraction at " + index + ": count=" + count + ", large=" + large + ", small=" + small);
      } finally {
        if (large != IntPtr.Zero) DestroyIcon(large);
        if (small != IntPtr.Zero) DestroyIcon(small);
      }
    }
  }
}
'@
}
$executablePath = (Resolve-Path -LiteralPath $Executable).Path
[ContainerIconResources]::Verify($executablePath, (Join-Path $repository 'src-tauri\icons\icon.ico'), (Join-Path $repository 'src-tauri\icons\project.ico'))
Write-Host 'Windows icons: 14 embedded frames match source bytes; app and CPROJ shell extraction passed.'
