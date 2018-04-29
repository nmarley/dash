// Copyright (c) 2014-2018 The Dash Core developers
// Distributed under the MIT/X11 software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include "privatesend.h"
#include "test/test_dash.h"

#include <boost/test/unit_test.hpp>

BOOST_FIXTURE_TEST_SUITE(privatesend_tests, BasicTestingSetup)

/* Test STUFF */
BOOST_AUTO_TEST_CASE(privatesend_test)
{
    CPrivateSend::InitStandardDenominations();

    int nCount;
    auto&& denoms = CPrivateSend::GetStandardDenominations();

    nCount = denoms.size();

    // ensure 4 standard denominations defined
    BOOST_CHECK_EQUAL(nCount, 4);

    // break something so that I know tests are running
    BOOST_CHECK_EQUAL(0, 1);
}

BOOST_AUTO_TEST_SUITE_END()
