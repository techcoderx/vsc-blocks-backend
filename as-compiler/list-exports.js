import { contract, reset, setContractImport } from '@vsc.eco/contract-testing-utils'
import { writeFile } from 'fs/promises'
import path from 'path'
import { fileURLToPath } from 'url'

setContractImport(import('./build/debug.js'))
await reset()

await writeFile(path.dirname(fileURLToPath(import.meta.url)) + '/build/exports.json', JSON.stringify(Object.keys(contract)))
