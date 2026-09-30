package com.zhoujun.awegit.system

import com.zhoujun.awegit.common.printError
import com.zhoujun.awegit.domain.usecases.OpenPathInSystemUseCase
import java.awt.Desktop
import java.net.URI
import javax.inject.Inject

private const val TAG = "SystemUtils"

// TODO This is not a domain use case, using a different name would be nice.
/**
 * Opens a URL in the default system browser
 */
class OpenUrlInBrowserUseCase @Inject constructor(
    private val openPathInSystemUseCase: OpenPathInSystemUseCase,
) {
    operator fun invoke(url: String) {
        if (!openPathInSystemUseCase(url)) {
            openUrlInBrowserJdk(url)
        }
    }


    private fun openUrlInBrowserJdk(url: String) {
        try {
            Desktop.getDesktop().browse(URI(url))
        } catch (ex: Exception) {
            printError(TAG, "Failed to open URL in browser")
            ex.printStackTrace()
        }
    }
}
