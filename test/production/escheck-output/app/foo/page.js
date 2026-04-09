'use client'

import { data } from '../tla'

async function fetchMore() {
  const res = await Promise.resolve('more data')
  return res
}

export default function Page() {
  return (
    <div>
      <button onClick={() => fetchMore().then(console.log)}>
        {data.message}
      </button>
    </div>
  )
}
