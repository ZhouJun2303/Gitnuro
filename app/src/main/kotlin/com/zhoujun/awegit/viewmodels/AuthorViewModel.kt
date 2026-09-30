package com.zhoujun.awegit.viewmodels

import com.zhoujun.awegit.TabViewModel
import com.zhoujun.awegit.domain.errors.Either
import com.zhoujun.awegit.domain.models.AuthorInfo
import com.zhoujun.awegit.domain.models.Identity
import com.zhoujun.awegit.domain.models.emptyIdentity
import com.zhoujun.awegit.domain.usecases.GetAuthorUseCase
import com.zhoujun.awegit.domain.usecases.SaveAuthorUseCase
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

class AuthorViewModel @Inject constructor(
    private val saveAuthorUseCase: SaveAuthorUseCase,
    private val getAuthorUseCase: GetAuthorUseCase,
) : TabViewModel() {
    val authorInfo: StateFlow<AuthorInfo> = flow {
        val author = when (val author = getAuthorUseCase()) {
            is Either.Ok -> author.value
            else -> AuthorInfo(emptyIdentity(), emptyIdentity())
        }

        emit(author)
    }
        .stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(),
            initialValue = AuthorInfo(emptyIdentity(), emptyIdentity()),
        )


    fun saveAuthorInfo(globalName: String?, globalEmail: String?, name: String?, email: String?) = viewModelScope.launch {
        saveAuthorUseCase(
            AuthorInfo(Identity(globalName, globalEmail), Identity(name, email))
        )
    }
}
