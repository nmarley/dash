// Copyright (c) 2018 The Dash Core developers
// Distributed under the MIT/X11 software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include "spork.h"
#include "test/test_dash.h"

#include <boost/test/unit_test.hpp>

// sporkManager.SetSporkAddress(GetArg("-sporkaddr", Params().SporkAddress()))
// sporkManager.SetPrivKey(GetArg("-sporkkey", ""))

BOOST_FIXTURE_TEST_SUITE(spork_tests, BasicTestingSetup)

/* Test calculation of next difficulty target with DGW */
BOOST_AUTO_TEST_CASE(get_next_work)
{
    SelectParams(CBaseChainParams::MAIN);
    const Consensus::Params& params = Params().GetConsensus();

    BOOST_CHECK_EQUAL(GetNextWorkRequired(&blockIndexLast, &blockHeader, paramsdev), 0x207fffff); // Block #123457 has 0x207fffff
}

BOOST_AUTO_TEST_SUITE_END()
