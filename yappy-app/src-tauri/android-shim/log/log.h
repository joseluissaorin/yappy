// Shim para fdk-aac en Android: el NDK no trae el log/log.h de AOSP que
// fdk-aac incluye bajo -DANDROID. Solo usa android_errorWriteLog para
// reportar streams malformados; aquí es un no-op.
#pragma once
#define android_errorWriteLog(tag, subTag) ((void)0)
