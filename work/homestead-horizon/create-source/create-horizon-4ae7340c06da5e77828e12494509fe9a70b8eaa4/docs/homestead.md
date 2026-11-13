# Homestead resource integration

This local integration targets the player's Homestead 2.0.1 Fabric 1.20.1
instance. Create's gameplay reference remains the pinned Create 6.0.10 source.
It does not establish complete behavioral parity with Fabric Create 6.0.8.1
or execute Java addon JARs.

The resource importer additionally accepts exactly
`create-fabric-6.0.8.1+build.1744-mc1.20.1.jar`, SHA-256
`a613603fb08d967c5a4138d1adb7374c4a347b91e489fe21579a4cff4f40105a`.
The original reference JAR remains supported.

The Fabric `porting_lib:obj` model declaration is translated to Horizon's
native NeoForge OBJ decoder while retaining the original OBJ, MTL, textures
and `flip_v` data. Unknown or ambiguous custom loaders are rejected. The
import receipt and CLI report identify the actual source version and hash.

The Fabric JAR has no top-level license file. Its `fabric.mod.json` is kept
as source metadata with a private-cache notice; no substitute license is
invented. Imported art, audio and JARs remain local and are not part of the
public source package.

Machine recipes, pack configuration changes and addon behavior require
separate integration and acceptance evidence. Trains and Steam 'n' Rails
are not implemented by this resource adapter.
