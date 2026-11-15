import{n as e}from"./chunk-gql-client-DiG1IC5F.js";var t=`query {
  fileIndexVolumes { id path isAvailable indexMode includedDirs excludedDirs lastScanAt lastScanStatus lastScanCount lastScanTotal }
  fileIndexDefaults { excludedDirs excludedNames excludedPackages excludedUserLibraries }
}`,n=`mutation($input: FileIndexVolumeInput!) {
  setFileIndexVolume(input: $input)
}`;async function r(t,r=e()){let{id:i,indexMode:a,includedDirs:o,excludedDirs:s}=t,c=await r(n,{input:{volumeId:i,indexMode:a,includedDirs:o,excludedDirs:s}});if(c.errors?.length)throw Error(c.errors[0].message);if(!c.data.setFileIndexVolume)throw Error(`Index configuration write refused`)}var i=`mutation($volumeId: ID) {
  rebuildFileIndex(volumeId: $volumeId)
}`;function a(e){return e.isAvailable&&(e.indexMode===`BLOCKLIST`||e.indexMode===`ALLOWLIST`&&e.includedDirs.length>0)}export{r as i,t as n,i as r,a as t};