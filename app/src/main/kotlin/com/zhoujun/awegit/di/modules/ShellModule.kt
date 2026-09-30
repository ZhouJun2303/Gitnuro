package com.zhoujun.awegit.di.modules

import com.zhoujun.awegit.AppEnvInfo
import com.zhoujun.awegit.common.OS
import com.zhoujun.awegit.common.currentOs
import com.zhoujun.awegit.domain.FlatpakShellManager
import com.zhoujun.awegit.domain.IShellManager
import com.zhoujun.awegit.domain.ShellManager
import com.zhoujun.awegit.terminal.ITerminalProvider
import com.zhoujun.awegit.terminal.LinuxTerminalProvider
import com.zhoujun.awegit.terminal.MacTerminalProvider
import com.zhoujun.awegit.terminal.WindowsTerminalProvider
import dagger.Module
import dagger.Provides
import javax.inject.Provider

@Module
class ShellModule {
    @Provides
    fun provideShellManager(
        appEnvInfo: AppEnvInfo,
        shellManager: Provider<ShellManager>,
        flatpakShellManager: Provider<FlatpakShellManager>,
    ): IShellManager {
        return if (appEnvInfo.isFlatpak)
            flatpakShellManager.get()
        else
            shellManager.get()
    }

    @Provides
    fun provideTerminalProvider(
        linuxTerminalProvider: Provider<LinuxTerminalProvider>,
        windowsTerminalProvider: Provider<WindowsTerminalProvider>,
        macTerminalProvider: Provider<MacTerminalProvider>,
    ): ITerminalProvider {
        return when (currentOs) {
            OS.LINUX -> linuxTerminalProvider.get()
            OS.WINDOWS -> windowsTerminalProvider.get()
            OS.MAC -> macTerminalProvider.get()
            OS.UNKNOWN -> throw NotImplementedError("Unknown operating system")
        }
    }
}