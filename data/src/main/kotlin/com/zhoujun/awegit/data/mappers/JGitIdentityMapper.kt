package com.zhoujun.awegit.data.mappers

import com.zhoujun.awegit.domain.models.Identity
import org.eclipse.jgit.lib.PersonIdent
import javax.inject.Inject

class JGitIdentityMapper @Inject constructor(): DataMapper<Identity, PersonIdent> {
    override fun toData(value: Identity): PersonIdent {
        return PersonIdent(
            value.name,
            value.email,
        )
    }

    override fun toDomain(value: PersonIdent): Identity {
       return with (value) {
           Identity(
               name = this.name,
               email = this.emailAddress,
           )
       }
    }
}