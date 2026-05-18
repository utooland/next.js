import foo from '#foo'
import dep from '#dep'
import depInNm from '#dep-in-nm'
import pattern from '#pattern/pat.js'
import conditionalImport from '#conditional'
const conditionalRequire = require('#conditional')

console.log(foo, dep, depInNm, pattern, conditionalImport, conditionalRequire)
