# AweGit Agent

## 自动提交

每次对话里只要改了仓库文件，收尾时自动提交一次。

- 只提交本次相关改动；没有改动就不提交
- 提交说明格式：`type: 描述`。类型用 `feat`、`fix`、`docs`、`style`、`refactor`、`test`、`chore`、`perf`、`art`、`revert`，一句话说明原因
- 不提交密钥、凭证、`.env`
- 不改 git config，不 force push，不 `--no-verify`，不 amend 已推送的提交

## 项目规范

Compose Desktop + JGit 的多平台 Git 客户端。主体是 Kotlin（JVM），少量 Rust 经 UniFFI 暴露给 Kotlin。

模块：

- `app`：界面、ViewModel、Dagger 装配
- `domain`：用例（`*UseCase`）、接口（`I*GitAction`）、模型；错误用 `Either`
- `data`：`*GitAction` 实现（JGit）和数据仓库
- `common`：共用工具、`TabScope`
- `rs`：Rust 绑定

约定：

- 包名 `com.zhoujun.awegit`
- 依赖注入用 Dagger：构造函数 `@Inject`，接口在 Module 里 `@Binds`
- 业务放 UseCase，Git 操作放 GitAction，界面不直接调 JGit
- 依赖版本写在 `gradle/libs.versions.toml`
- JVM toolchain 25；测试用 JUnit 5 + MockK
- 运行 `./gradlew run`，测试 `./gradlew test`
