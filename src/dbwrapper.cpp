// Copyright (c) 2012-2015 The Bitcoin Core developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include "dbwrapper.h"

#include "util.h"
#include "random.h"

#include <boost/filesystem.hpp>

#include <rocksdb/db.h>
#include <rocksdb/cache.h>
#include <rocksdb/table.h>
#include <rocksdb/env.h>
#include <rocksdb/filter_policy.h>
#include <memenv.h>
#include <stdint.h>

void HandleError(const rocksdb::Status& status) throw(dbwrapper_error)
{
    if (status.ok())
        return;
    LogPrintf("%s\n", status.ToString());
    if (status.IsCorruption())
        throw dbwrapper_error("Database corrupted");
    if (status.IsIOError())
        throw dbwrapper_error("Database I/O error");
    if (status.IsNotFound())
        throw dbwrapper_error("Database entry missing");
    throw dbwrapper_error("Unknown database error");
}

static rocksdb::Options GetOptions(size_t nCacheSize)
{
    rocksdb::Options options;
    rocksdb::BlockBasedTableOptions table_options;
    table_options.block_cache = rocksdb::NewLRUCache(nCacheSize / 2);
    table_options.filter_policy.reset(rocksdb::NewBloomFilterPolicy(10));

    options.table_factory.reset(rocksdb::NewBlockBasedTableFactory(table_options));
    options.write_buffer_size = nCacheSize / 4; // up to two write buffers may be held in memory simultaneously
    options.compression = rocksdb::kNoCompression;
    options.max_open_files = 64;
    options.paranoid_checks = true;

    return options;
}

CDBWrapper::CDBWrapper(const boost::filesystem::path& path, size_t nCacheSize, bool fMemory, bool fWipe, bool obfuscate)
{
    penv = NULL;
    readoptions.verify_checksums = true;
    iteroptions.verify_checksums = true;
    iteroptions.fill_cache = false;
    syncoptions.sync = true;
    options = GetOptions(nCacheSize);
    options.create_if_missing = true;
    if (fMemory) {
        penv = rocksdb::NewMemEnv(rocksdb::Env::Default());
        options.env = penv;
    } else {
        if (fWipe) {
            LogPrintf("Wiping LevelDB in %s\n", path.string());
            rocksdb::Status result = rocksdb::DestroyDB(path.string(), options);
            HandleError(result);
        }
        TryCreateDirectory(path);
        LogPrintf("Opening LevelDB in %s\n", path.string());
    }
    rocksdb::Status status = rocksdb::DB::Open(options, path.string(), &pdb);
    HandleError(status);
    LogPrintf("Opened LevelDB successfully\n");

    // The base-case obfuscation key, which is a noop.
    obfuscate_key = std::vector<unsigned char>(OBFUSCATE_KEY_NUM_BYTES, '\000');

    bool key_exists = Read(OBFUSCATE_KEY_KEY, obfuscate_key);

    if (!key_exists && obfuscate && IsEmpty()) {
        // Initialize non-degenerate obfuscation if it won't upset
        // existing, non-obfuscated data.
        std::vector<unsigned char> new_key = CreateObfuscateKey();

        // Write `new_key` so we don't obfuscate the key with itself
        Write(OBFUSCATE_KEY_KEY, new_key);
        obfuscate_key = new_key;

        LogPrintf("Wrote new obfuscate key for %s: %s\n", path.string(), GetObfuscateKeyHex());
    }

    LogPrintf("Using obfuscation key for %s: %s\n", path.string(), GetObfuscateKeyHex());
}

CDBWrapper::~CDBWrapper()
{
    delete pdb;
    pdb = NULL;
    rocksdb::BlockBasedTableOptions table_options;
    table_options.filter_policy = nullptr;
    table_options.block_cache = nullptr;
    options.table_factory.reset(rocksdb::NewBlockBasedTableFactory(table_options));
    delete penv;
    options.env = NULL;
}

bool CDBWrapper::WriteBatch(CDBBatch& batch, bool fSync) throw(dbwrapper_error)
{
    rocksdb::Status status = pdb->Write(fSync ? syncoptions : writeoptions, &batch.batch);
    HandleError(status);
    return true;
}

// Prefixed with null character to avoid collisions with other keys
//
// We must use a string constructor which specifies length so that we copy
// past the null-terminator.
const std::string CDBWrapper::OBFUSCATE_KEY_KEY("\000obfuscate_key", 14);

const unsigned int CDBWrapper::OBFUSCATE_KEY_NUM_BYTES = 8;

/**
 * Returns a string (consisting of 8 random bytes) suitable for use as an
 * obfuscating XOR key.
 */
std::vector<unsigned char> CDBWrapper::CreateObfuscateKey() const
{
    unsigned char buff[OBFUSCATE_KEY_NUM_BYTES];
    GetRandBytes(buff, OBFUSCATE_KEY_NUM_BYTES);
    return std::vector<unsigned char>(&buff[0], &buff[OBFUSCATE_KEY_NUM_BYTES]);

}

bool CDBWrapper::IsEmpty()
{
    boost::scoped_ptr<CDBIterator> it(NewIterator());
    it->SeekToFirst();
    return !(it->Valid());
}

const std::vector<unsigned char>& CDBWrapper::GetObfuscateKey() const
{
    return obfuscate_key;
}

std::string CDBWrapper::GetObfuscateKeyHex() const
{
    return HexStr(obfuscate_key);
}

CDBIterator::~CDBIterator() { delete piter; }
bool CDBIterator::Valid() { return piter->Valid(); }
void CDBIterator::SeekToFirst() { piter->SeekToFirst(); }
void CDBIterator::Next() { piter->Next(); }
