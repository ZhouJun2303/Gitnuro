package com.zhoujun.awegit.di.modules

import com.zhoujun.awegit.common.TabScope
import com.zhoujun.awegit.domain.TabCoroutineScope
import dagger.Module
import dagger.Provides
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob

@Module
class TabModule {    @TabScope
    @Provides
    fun provideTabCoroutineScope() = TabCoroutineScope()
}
