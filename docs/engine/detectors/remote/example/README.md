# Detector Remoto — provedor fictício `example`

**Lugar reservado para documentação futura.** A implementação especializada observaria mudanças desse provedor e emitiria eventos individuais segundo a [interface do Detector Remoto](../INTERFACES.md), usando a [API específica](../../../providers/example/api/README.md).

Não há implementação ou testes.

## Contrato esperado

O exemplo de provedor seguirá o **mesmo `models::File`**, encapsulando `FileData::Remote(RemoteFile)` produzido por adaptador próprio (sem adicionar variante `FileData::Example`). `RemoteFile.reference` usa a conta + ID do provedor e `provider_data` guarda metadados específicos se autorizados. A API do Detector Remoto permanece estável, com um `File` por emissão e sem decisões de exclusão baseadas em aviso ambíguo. Este é apenas um exemplo documental sem implementação.
