# Configuration — INTERFACES

## Objetivo

**Contrato conceitual proposto**, ainda não implementado. Existem duas responsabilidades **separadas**, sem vazamento entre elas: preferências tipadas opcionais e serviço técnico comum de backup/restauração SQLite. Configuration não possui banco de dados de domínio, filas ou regras da engine.

## 1. Preferências — contratos propostos

```rust
pub enum PreferenceScope {
    Global,
    Sync(SyncId),
}
pub enum PreferenceKey {
    RemotePollInterval,
    RemotePageBudget,
    LocalDebounce,
    DetectorEventBatchSize,
    NetworkRequestTimeout,
    NetworkRetryDelay,
}
pub enum PreferenceValue {
    Duration(Duration),
    PositiveInteger(u32),
}
pub struct PreferenceRequest {
    pub scope: PreferenceScope,
    pub key: PreferenceKey,
}
pub struct PreferenceResult {
    pub override_value: Option<PreferenceValue>,
}
pub struct SetPreferenceRequest {
    pub scope: PreferenceScope,
    pub key: PreferenceKey,
    pub value: PreferenceValue,
}
pub struct ClearPreferenceRequest {
    pub scope: PreferenceScope,
    pub key: PreferenceKey,
}
```

`None` é **ausência de substituição**: o valor padrão pertence ao consumidor. Uma implementação de `resolve` pode obter primeiro o override de sincronização (quando permitido) e depois o global, mas **não fixa o default** do módulo. Os escopos permitidos são por chave, validados antes da gravação. O nome conceitual de preferência **não é um caminho físico** e não cria dependência de configurações para os demais módulos.

```rust
impl Configuration {
    pub fn new(config: ConfigurationConfig) -> ConfigResult<Self>;
    pub fn get(&self, request: PreferenceRequest) -> ConfigResult<PreferenceResult>;
    pub fn set(&self, request: SetPreferenceRequest) -> ConfigResult<()>;
    pub fn clear(&self, request: ClearPreferenceRequest) -> ConfigResult<()>;
}
```

Os métodos de leitura/escrita são síncronos e limitados a preferências. Não executar rede, SQLite de outros módulos, detecção ou operações de arquivos. O consumidor decide se e quando aplica novo valor. Notificação de mudança não é necessária à primeira versão.

## 2. Recuperação SQLite — contratos internos

O mecanismo é reutilizável **para bancos explicitamente registrados por seus Storages proprietários**, sem descobrir caminhos, abrir tabelas de domínio ou impor um banco único. As assinaturas abaixo são **esboços de integração**, não tipos compilados:

```rust
pub struct DatabaseRegistration {
    pub identity: DatabaseIdentity, // inclui classe do banco e identidade de sincronização
    pub location: AuthorizedDatabaseLocation, // restrita à recuperação
    pub access: DatabaseQuiescencePort, // exclusividade de troca e validação pelo proprietário
}
pub enum RecoveryOutcome {
    Restored,
    RecoveryFailed,
}
pub struct BackupRequest { pub identity: DatabaseIdentity }
pub struct RestoreRequest { pub identity: DatabaseIdentity }
pub struct BackupResult { pub snapshot: OpaqueSnapshotId }
pub struct RestoreResult { pub outcome: RecoveryOutcome }

impl Configuration {
    pub fn register_database(&self, request: DatabaseRegistration) -> ConfigResult<()>;
    pub async fn backup_database(&self, request: BackupRequest) -> ConfigResult<BackupResult>;
    pub async fn restore_database(&self, request: RestoreRequest) -> ConfigResult<RestoreResult>;
}
```

`DatabaseQuiescencePort` é uma capacidade restrita de **coordenação de acesso**, não um handle SQL exposto. Seu contrato deve garantir que, antes da substituição, todos os escritores/leitores e conexões WAL relevantes estejam inativos; a validação de identidade/schema após reabrir é feita pelo Storage. O implementador pode usar uma integração interna em vez desses tipos públicos definitivos. **Nunca permitir restauração com conexões ativas**.

A criação do snapshot utiliza a Online Backup API SQLite, ou técnica oficial equivalente verificada (`VACUUM INTO`). Durante a criação de backup online, as regras de conexão da API são respeitadas; para restaurar **um banco de arquivo ativo**, obter exclusividade e preservar os arquivos anteriores antes de substituir. O backup precisa pertencer à mesma identidade de armazenamento. Não é seguro considerar o caminho físico, isoladamente, como prova de identidade.

### Falhas e segurança

- Indisponibilidade transitória, `SQLITE_BUSY`, permissões, disco cheio, conta errada ou esquema não suportado **não** disparam substituição automática.
- Corrupção comprovada pode solicitar restauração; se não houver cópia íntegra ou não for possível fechar o banco, devolver falha sem substituir o original.
- A cópia danificada deve permanecer disponível para diagnóstico, com arquivos auxiliares importantes.
- `restore_database` não cria schema de Inventory/Detector e não decide quando usar Scanner/Analyzer; depois da substituição, o Storage precisa validar e autorizar o retorno à operação.
- Reconstrução vazia **não é** restauração. Depende de autorização explícita e inicialização própria do Storage; não converter perda de histórico em prova de exclusão.
- Credenciais Secret Service/keyring estão fora desse serviço.

## 3. Decisões deixadas para implementação

As assinaturas são contrato-alvo: ownership dos handles, runtime async, origem exata de snapshots, geração de IDs, diretórios de backup, WAL, estratégia de lock, quotas de retenção e limites concretos de cada preferência serão decididos com testes. Não impor threads, limite global de contas, nem número de transferências simultâneas nesta fase.
