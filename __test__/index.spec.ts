import test from 'ava'

import { Document } from '../index.js'

test('create element', (t) => {
  const doc = new Document()
  /*const element =*/
  doc.createElement('div', [{ name: 'scope-id', value: '123' }])
  const node_1 = doc.getNode(1)
  const node_2 = doc.getNode(1)
  console.log(node_1, node_2)
  t.is(node_1, node_2)
})
