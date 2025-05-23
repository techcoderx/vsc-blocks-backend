import { console, getEnv } from '@vsc.eco/sdk/assembly'

/**
 * Dumps the values of getEnv().
 * @param payload Anything
 * @returns Dump complete
 */
export function dumpEnv(payload: string): string {
  console.log('Begin dumping env')
  const env = getEnv()
  console.log(`Anchor block ${env.anchor_block}`)
  console.log(`Anchor height ${env.anchor_height}`)
  console.log(`Anchor id ${env.anchor_id}`)
  console.log(`Anchor timestamp ${env.anchor_timestamp}`)
  console.log(`Sender ${env.msg_sender}`)
  console.log(`Tx Origin ${env.tx_origin}`)
  return 'Dump complete'
}
