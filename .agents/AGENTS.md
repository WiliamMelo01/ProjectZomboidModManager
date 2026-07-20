# Regras de Desenvolvimento (PZMM)

- **Compatibilidade Multiplataforma**: Garanta sempre que qualquer código ou comando adicionado não interfira com o ambiente Linux. Mantenha os comandos remotos estritamente compatíveis com Linux (ex: SSH e helper remotos) e use diretivas de compilação condicional (`#[cfg(windows)]`, `cfg!(windows)`) ou blocos separados para comportamentos específicos do Windows local.
