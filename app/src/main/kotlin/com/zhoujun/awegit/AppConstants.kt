package com.zhoujun.awegit

object AppConstants {
    val openSourceProjects = listOf(
        Project("Google Dagger", "https://dagger.dev/", apache__2_0),
        Project("Compose Multiplatform", "https://www.jetbrains.com/lp/compose-mpp/", apache__2_0),
        Project("JGit", "https://www.eclipse.org/jgit/", edl),
        Project("JUnit 5", "https://junit.org/junit5/", edl),
        Project("Kotlin", "https://kotlinlang.org/", apache__2_0),
        Project(
            "Kotlinx.serialization",
            "https://kotlinlang.org/docs/serialization.html#example-json-serialization",
            apache__2_0
        ),
        Project("Mockk", "https://mockk.io/", apache__2_0),
        Project("LibSSH", "libssh.org/", lgpl__2_1),
        Project("Gitnuro", "https://github.com/JetpackDuba/Gitnuro", gpl__3_0),
    )


    // Remember to update build.gradle when changing this
    const val APP_NAME = "AweGit"
    const val APP_DESCRIPTION = "AweGit is a fast Git client inspired by Fork, with workspaces, AI generated commit messages and a live view of your repositories."
    const val APP_VERSION = "2.0-beta03"
    const val APP_VERSION_CODE = 25
    const val APP_AUTHOR = "ZhouJun"
    const val REPOSITORY_URL = "https://github.com/ZhouJun2303/AweGit"
    const val ISSUES_URL = "$REPOSITORY_URL/issues"
    const val RELEASES_URL = "$REPOSITORY_URL/releases"
    const val VERSION_CHECK_URL = "https://raw.githubusercontent.com/ZhouJun2303/AweGit/main/latest.json"
}


private val apache__2_0 = License("Apache 2.0", "https://www.apache.org/licenses/LICENSE-2.0")
private val edl = License("EDL", "https://www.eclipse.org/org/documents/edl-v10.php")
private val lgpl__2_1 = License("LGPL-2.1", "https://www.gnu.org/licenses/old-licenses/lgpl-2.1.en.html")
private val gpl__3_0 = License("GPL-3.0", "https://www.gnu.org/licenses/gpl-3.0.en.html")

data class License(
    val name: String,
    val url: String,
)

data class Project(val name: String, val url: String, val license: License)


