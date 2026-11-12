export function aadPurposes(source) {
  return Object.fromEntries(['AadPurpose', 'ResourcePurpose', 'LoginPurpose'].map(name => {
    const body = new RegExp(`impl ${name} \\{([\\s\\S]*?)\\n\\}`).exec(source)?.[1] ?? ''
    const entries = [...body.matchAll(/Self::(\w+)(?:\s*\{[^}]*\})?\s*=>\s*"([A-Z_]+)"/g)].map(([, key, value]) => [key, value])
    if (!entries.length || new Set(entries.map(([, value]) => value)).size !== entries.length) throw new Error(`Invalid ${name}`)
    return [name, Object.fromEntries(entries)]
  }))
}

export function aadPurposeModule(source) {
  return '// Generated from plain-core AAD purpose definitions.\n' + Object.entries(aadPurposes(source)).map(([name, values]) =>
    `export enum ${name} {\n${Object.entries(values).map(([key, value]) => `  ${key} = '${value}',`).join('\n')}\n}\n`,
  ).join('\n')
}
