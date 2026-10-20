# Modelo de domínio — múltiplas contas em um motor

## Hierarquia de identidade e ownership

```text
DriveSyncEngine (um processo, uma biblioteca)
├── Account(account_id, provider_subject, display_email, auth_state)
│   ├── CredentialRef(account_id)          # tokens privados em secret storage
│   ├── ChangeCursor(account_id, change_scope)
│   └── SyncRoot(root_id, account_id, remote_folder_id, local_path, policy)
│       ├── FileIdentity(account_id, remote_file_id, root_id, local_id?, relative_path)
│       ├── BaseVersion(remote_version?, local_fingerprint?)
│       ├── SyncOperation(op_id, account_id, root_id, file_id, state, attempts)
│       └── Conflict(account_id, root_id, file_id, preserved_versions)
└── Scheduler: fila/limites globais, políticas por conta e por raiz
```

A identificação estável da conta não depende do texto do e-mail (que pode mudar), e o `remote_file_id` não é uma identidade global **no banco do DriveSync** sem `account_id`. `SyncRootId` pode ser único internamente, mas **cada operação deve conservar AccountId + SyncRootId** e validar a associação. Modelos não carregam tokens.

## Observação e decisão

`Observation(status=PRESENT|ABSENT|UNKNOWN, revision?, hash?, size?, mtime?)`; `Plan(action, expected_local, expected_remote, dependencies, destructive)`; `ChangeEvent(account_id, scope, identity, kind, seq)`.

`RemoteFileId` deve ser tratado como identidade, independentemente do nome ou caminho. Falha remota significa `UNKNOWN`, não `ABSENT`.

## Semântica dos cursores

Tokens do `changes` pertencem ao **escopo da API que os emitiu** (usuário/Drive compartilhado e parâmetros aplicáveis), não automaticamente a uma pasta local. Não reusar token de uma conta com outra nem duplicar erroneamente um cursor para duas raízes que consomem o mesmo feed; resolver o mapeamento entre alterações e raízes.

## Invariantes de isolamento

- Um token e uma sessão OAuth **por conta** (ou armazenamento equivalente com separação estrita).
- A conta A nunca usa cursor, versão-base, fila ou operação da B.
- Caminhos locais de duas raízes ativas **não podem ser iguais nem se sobrepor**, independentemente da conta.
- Pausar/desconectar A não interrompe B; credencial revogada de A não dispara exclusões.
- Uma falha de banco global pode degradar o **motor inteiro** para manter integridade; isto não é confundido com uma falha limitada de autenticação/rede de A.
- Índices e chaves estrangeiras protegem o contexto de conta nas relações; transações devem validar `root.account_id == operation.account_id`.
- Um único motor pode executar sessões concorrentes com limite compartilhado de recursos e limites por conta.
