import{ct as e}from"./chunk-http-DIOZRrUf.js";import{t}from"./chunk-gql-client-BBjM_98S.js";var n=`query {
  fileIndexVolumes { id path isAvailable indexMode includedDirs excludedDirs lastScanAt lastScanStatus lastScanCount lastScanTotal }
  fileIndexDefaults { excludedDirs excludedNames excludedProfileDirs { profilesDir excludedDir exceptDirs } mediaExcludedDirSuffixes }
}`,r=`mutation($volumeId: ID!, $input: FileIndexVolumeInput!) {
  setFileIndexVolume(volumeId: $volumeId, input: $input)
}`;async function i(n,i=t()){let{id:a,indexMode:o,includedDirs:s,excludedDirs:c}=n,l=await i(r,{volumeId:a,input:{indexMode:o,includedDirs:s,excludedDirs:c}});if(l.errors?.length)throw e.fromGraphql(l.errors[0]);if(!l.data.setFileIndexVolume)throw Error(`Index configuration write refused`)}var a=`mutation($volumeId: ID) {
  rebuildFileIndex(volumeId: $volumeId)
}`;function o(e){return e.isAvailable&&(e.indexMode===`BLOCKLIST`||e.indexMode===`ALLOWLIST`&&e.includedDirs.length>0)}export{i,n,a as r,o as t};