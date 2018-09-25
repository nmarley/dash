// Copyright (c) 2018 The Dash Core developers

#include "spork.h"
#include "string.h"
#include "chainparams.h"
#include "masternode.h"

#include "test/test_dash.h"
#include <boost/test/unit_test.hpp>

BOOST_FIXTURE_TEST_SUITE(mnp_tests, TestChain100Setup)

// private key for this WIF / address:
// 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
std::string key = "cTJTN1hGHqucsgqmYVbhU3g4eU9g5HzE1sxuSY32M1xap1K4sYHF";
std::string addr = "yMtMWAjPhUquwKtdG4wzj9Cpn4asQkLV8F";

BOOST_AUTO_TEST_CASE(MNP_GetHash)
{
    COutPoint x = COutPoint(uint256S("0x147caa76786596590baa4e98f5d9f48b86c7765e489f7a6ff3360fe5c674360b"), 0);
    const COutPoint& ref = x;
    CMasternodePing mnp = CMasternodePing(ref);
    mnp.sigTime = 1537851983;

    //     CHashWriter ss(SER_GETHASH, PROTOCOL_VERSION);
    //     ss << masternodeOutpoint << uint8_t{} << 0xffffffff; // adding dummy values here to match old hashing format
    //     ss << sigTime;
    //     return ss.GetHash();

    // debugging
    //     ss << masternodeOutpoint << uint8_t{} << 0xffffffff; // adding dummy values here to match old hashing format
    // std::cout << "NGM DEBUGGING: mnp.masternodeOutpoint: " << mnp.masternodeOutpoint << std::endl;

    // class COutPoint
    // public:
    //     uint256 hash;
    //     uint32_t n;

    // NGM DEBUGGING: mnp.masternodeOutpoint.hash: 147caa76786596590baa4e98f5d9f48b86c7765e489f7a6ff3360fe5c674360b
    // NGM DEBUGGING: mnp.masternodeOutpoint.n: 0
    // NGM DEBUGGING: mnp.sigTime: 1537851983
    // NGM DEBUGGING: mnp.GetHash(): 12a5ec4710ab632b7f56cc8c563b05017a4ad6f68f282da541330759a1ef24fc

    // std::cout << "NGM DEBUGGING: mnp.masternodeOutpoint.hash: " << mnp.masternodeOutpoint.hash.ToString() << std::endl;
    // std::cout << "NGM DEBUGGING: mnp.masternodeOutpoint.n: " << mnp.masternodeOutpoint.n << std::endl;
    // std::cout << "NGM DEBUGGING: mnp.sigTime: " << mnp.sigTime << std::endl;
    // std::cout << "NGM DEBUGGING: mnp.GetHash(): " << mnp.GetHash().ToString() << std::endl;

    uint256 expected = uint256S("0x12a5ec4710ab632b7f56cc8c563b05017a4ad6f68f282da541330759a1ef24fc");
    BOOST_CHECK(mnp.GetHash() == expected);
}

BOOST_AUTO_TEST_SUITE_END()
