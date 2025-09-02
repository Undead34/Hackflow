# Hackflow

Motor de orquestación de flujos de trabajo escrito en **Rust** con scripts de
**Lua**. Permite definir tareas que se ejecutan de forma secuencial o en
paralelo utilizando un grafo acíclico dirigido.

## Características

- Sistema de tareas extensible con soporte para dependencias.
- Paso de datos entre tareas: la salida de una tarea puede modificar el
  comportamiento de la siguiente. Por ejemplo, `RunCommandTask` ahora añade
  argumentos basados en resultados de tareas previas como `DnsLookupTask`.
